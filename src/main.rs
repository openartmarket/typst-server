use axum::{
    Json, Router,
    body::Body,
    extract::Request,
    extract::{DefaultBodyLimit, Multipart},
    http::{Response, StatusCode, header::AUTHORIZATION},
    middleware::{self, Next},
    response::IntoResponse,
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::ops::Range;
use tower_http::limit::RequestBodyLimitLayer;
use typst::diag::{Severity, SourceDiagnostic};
use typst::foundations::{Bytes, Dict, IntoValue};
use typst::syntax::{FileId, Source, Span, VirtualPath};
use typst_as_lib::{TypstAsLibError, TypstEngine, typst_kit_options::TypstKitFontOptions};

#[tokio::main]
async fn main() {
    let host = std::env::var("HOST").unwrap_or("0.0.0.0".to_string());
    let port = std::env::var("PORT").unwrap_or("3009".to_string());
    let token = std::env::var("TYPST_SERVER_TOKEN").ok();
    let auth_enabled = token.is_some();

    let mut app = Router::new().route("/", post(create_pdf));
    if let Some(token) = token {
        app = app.layer(middleware::from_fn_with_state(token, auth_middleware));
    }
    let app = app
        .route("/version", get(version))
        .layer(DefaultBodyLimit::disable())
        .layer(RequestBodyLimitLayer::new(
            250 * 1024 * 1024, /* 250mb */
        ));

    let address = format!("{}:{}", host, port);
    let listener = match tokio::net::TcpListener::bind(&address).await {
        Ok(listener) => listener,
        Err(e) => {
            eprintln!("typst-server: cannot listen on {}: {}", address, e);
            std::process::exit(1);
        }
    };

    match auth_enabled {
        true => println!("typst-server running on http://{}:{} (auth enabled)", host, port),
        false => println!(
            "typst-server running on http://{}:{} (auth disabled — TYPST_SERVER_TOKEN not set)",
            host, port
        ),
    }

    if let Err(e) = axum::serve(listener, app).await {
        eprintln!("typst-server: server error: {}", e);
        std::process::exit(1);
    }
}

async fn auth_middleware(
    axum::extract::State(token): axum::extract::State<String>,
    request: Request<Body>,
    next: Next,
) -> Result<Response<Body>, (StatusCode, &'static str)> {
    let auth_header = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|header| header.to_str().ok())
        .ok_or((StatusCode::UNAUTHORIZED, "Missing Authorization header"))?;

    if !auth_header.starts_with("Basic ") {
        return Err((StatusCode::UNAUTHORIZED, "Invalid Authorization format"));
    }

    let expected = BASE64.encode(format!(":{}", token));
    let provided = auth_header.trim_start_matches("Basic ").trim();

    if provided != expected {
        return Err((StatusCode::UNAUTHORIZED, "Invalid credentials"));
    }

    Ok(next.run(request).await)
}

async fn version() -> impl IntoResponse {
    Json(json!({ "version": env!("CARGO_PKG_VERSION") }))
}

async fn create_pdf(mut multipart: Multipart) -> impl IntoResponse {
    let mut template_content = None;
    let mut json_data = None;
    let mut data_map: HashMap<String, Bytes> = HashMap::new();
    let mut fonts: Vec<Bytes> = Vec::new();
    let mut additional_sources: HashMap<String, String> = HashMap::new();

    loop {
        // A malformed body is the client's mistake, so report it rather than
        // panicking the request task.
        let field = match multipart.next_field().await {
            Ok(Some(field)) => field,
            Ok(None) => break,
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    format!("Malformed multipart body: {}", e),
                )
                    .into_response();
            }
        };
        // An unnamed part can still be classified by its content type or
        // filename, so treat a missing name the same as an empty one.
        let name = field.name().unwrap_or("").to_string();
        let file_name = field.file_name().unwrap_or("").to_string();
        let content_type = field.content_type().unwrap_or("").to_string();
        let data = match field.bytes().await {
            Ok(data) => data,
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    format!("Could not read multipart field \"{}\": {}", name, e),
                )
                    .into_response();
            }
        };

        if name == "template" {
            // The filename, when the client sends one, is only used to name the
            // main file in diagnostics.
            let main_name = match file_name.is_empty() {
                true => "main.typ".to_string(),
                false => file_name.clone(),
            };
            template_content = Some((main_name, String::from_utf8_lossy(&data).to_string()));
        } else if name == "data" {
            match serde_json::from_slice::<Value>(&data) {
                Ok(parsed_data) => json_data = Some(parsed_data),
                Err(e) => {
                    return (StatusCode::BAD_REQUEST, format!("Invalid JSON data: {}", e))
                        .into_response();
                }
            }
        } else if content_type == "image/png"
            || content_type == "image/jpeg"
            || content_type == "image/gif"
            || content_type == "image/svg+xml"
        {
            data_map.insert(name.clone(), Bytes::new(data));
        } else if file_name.ends_with(".otf") || file_name.ends_with(".ttf") {
            fonts.push(Bytes::new(data));
        } else if file_name.ends_with(".typ") {
            additional_sources.insert(
                file_name.clone(),
                String::from_utf8_lossy(&data).to_string(),
            );
        }
    }

    let (main_name, template_string) = match template_content {
        Some(content) => content,
        None => return (StatusCode::BAD_REQUEST, "No template provided").into_response(),
    };

    let data = match json_data {
        Some(data) => data,
        None => return (StatusCode::BAD_REQUEST, "No data provided").into_response(),
    };

    let typst_data = json_to_typst_value(data, &data_map);

    // Keep the parsed sources so compile errors can be reported as
    // file:line:column with an excerpt, rather than an opaque span number.
    let main_source = Source::new(
        FileId::new(None, VirtualPath::new(&main_name)),
        template_string,
    );
    let extra_sources: Vec<Source> = additional_sources
        .iter()
        .map(|(name, text)| Source::new(FileId::new(None, VirtualPath::new(name)), text.clone()))
        .collect();
    let sources: HashMap<FileId, Source> = std::iter::once(main_source.clone())
        .chain(extra_sources.iter().cloned())
        .map(|source| (source.id(), source))
        .collect();

    let template = TypstEngine::builder()
        .main_file(main_source)
        .with_static_source_file_resolver(extra_sources)
        .search_fonts_with(
            TypstKitFontOptions::default()
                .include_system_fonts(true)
                // This line is not necessary, because thats the default.
                .include_embedded_fonts(true),
        )
        .fonts(fonts)
        .build();

    let doc = match template.compile_with_input(typst_data).output {
        Ok(doc) => doc,
        Err(TypstAsLibError::TypstSource(diagnostics)) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format_diagnostics(&diagnostics, &sources),
            )
                .into_response();
        }
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    let options = Default::default();
    let pdf = match typst_pdf::pdf(&doc, &options) {
        Ok(pdf) => pdf,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("{:?}", e)).into_response(),
    };

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/pdf")
        .body(Body::from(pdf))
        .unwrap()
}

/// Longest source excerpt shown under a diagnostic, in characters. Generated
/// documents routinely have lines thousands of characters long, so the excerpt
/// is a window centred on the offending column rather than the whole line.
const MAX_EXCERPT_CHARS: usize = 160;

/// Render compile diagnostics the way the `typst` CLI does: severity, message,
/// and `file:line:column` with a source excerpt. Typst identifies positions by
/// an opaque `Span` number, which is meaningless without the sources that
/// produced it.
fn format_diagnostics(diagnostics: &[SourceDiagnostic], sources: &HashMap<FileId, Source>) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| format_diagnostic(diagnostic, sources))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn format_diagnostic(diagnostic: &SourceDiagnostic, sources: &HashMap<FileId, Source>) -> String {
    let severity = match diagnostic.severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
    };
    let mut out = format!("{}: {}", severity, diagnostic.message);
    if let Some(excerpt) = format_span_excerpt(diagnostic.span, sources) {
        out.push_str(&excerpt);
    }
    for hint in &diagnostic.hints {
        out.push_str(&format!("\n  = hint: {}", hint));
    }
    for tracepoint in &diagnostic.trace {
        let location = match format_span_location(tracepoint.span, sources) {
            Some(location) => format!(" at {}", location),
            None => String::new(),
        };
        out.push_str(&format!("\n  = {}{}", tracepoint.v, location));
    }
    out
}

fn format_span_excerpt(span: Span, sources: &HashMap<FileId, Source>) -> Option<String> {
    let (source, range) = resolve_span(span, sources)?;
    let (line, column) = source.lines().byte_to_line_column(range.start)?;
    let line_range = source.lines().line_to_range(line)?;
    let text = source
        .text()
        .get(line_range)?
        .trim_end_matches(['\n', '\r']);
    Some(format!(
        "\n  --> {}:{}:{}\n   |\n   | {}\n   |",
        file_name(source.id()),
        line + 1,
        column + 1,
        excerpt_around(text, column)
    ))
}

fn format_span_location(span: Span, sources: &HashMap<FileId, Source>) -> Option<String> {
    let (source, range) = resolve_span(span, sources)?;
    let (line, column) = source.lines().byte_to_line_column(range.start)?;
    Some(format!("{}:{}:{}", file_name(source.id()), line + 1, column + 1))
}

/// Typst source files use numbered spans, which only the parsed `Source` can
/// resolve; other files carry the byte range in the span itself.
fn resolve_span(
    span: Span,
    sources: &HashMap<FileId, Source>,
) -> Option<(&Source, Range<usize>)> {
    let source = sources.get(&span.id()?)?;
    let range = source.range(span).or_else(|| span.range())?;
    Some((source, range))
}

fn excerpt_around(text: &str, column: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= MAX_EXCERPT_CHARS {
        return text.to_string();
    }
    let centred = column.saturating_sub(MAX_EXCERPT_CHARS / 2);
    let end = (centred + MAX_EXCERPT_CHARS).min(chars.len());
    let start = end.saturating_sub(MAX_EXCERPT_CHARS);
    let mut out = String::new();
    if start > 0 {
        out.push('…');
    }
    out.extend(&chars[start..end]);
    if end < chars.len() {
        out.push('…');
    }
    out
}

fn file_name(id: FileId) -> String {
    id.vpath().as_rootless_path().display().to_string()
}

// Convert serde_json::Value to typst::Dict, replacing "data:*" strings with base64 values
fn json_to_typst_value(value: Value, data_map: &HashMap<String, Bytes>) -> Dict {
    match value {
        Value::Object(map) => {
            let mut dict = Dict::new();
            for (k, v) in map {
                dict.insert(k.into(), json_value_to_typst_value(v, data_map));
            }
            dict
        }
        _ => {
            let mut dict = Dict::new();
            dict.insert("data".into(), json_value_to_typst_value(value, data_map));
            dict
        }
    }
}

fn json_value_to_typst_value(
    value: Value,
    data_map: &HashMap<String, Bytes>,
) -> typst::foundations::Value {
    use typst::foundations::{Array, Value as TypstValue};

    match value {
        Value::Null => TypstValue::None,
        Value::Bool(b) => b.into_value(),
        Value::Number(n) => {
            if n.is_i64() {
                n.as_i64().unwrap().into_value()
            } else if n.is_u64() {
                n.as_u64().unwrap().into_value()
            } else {
                n.as_f64().unwrap().into_value()
            }
        }
        Value::String(s) => {
            if let Some(bytes) = data_map.get(&s) {
                bytes.clone().into_value()
            } else {
                s.into_value()
            }
        }
        Value::Array(arr) => {
            let mut typst_arr = Array::new();
            for item in arr {
                typst_arr.push(json_value_to_typst_value(item, data_map));
            }
            TypstValue::Array(typst_arr)
        }
        Value::Object(map) => {
            let mut dict = Dict::new();
            for (k, v) in map {
                dict.insert(k.into(), json_value_to_typst_value(v, data_map));
            }
            TypstValue::Dict(dict)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::http::Request;
    use tower::ServiceExt;
    use typst::layout::PagedDocument;

    async fn post_body(content_type: &str, body: &'static str) -> (StatusCode, String) {
        let app = Router::new().route("/", post(create_pdf));
        let request = Request::builder()
            .method("POST")
            .uri("/")
            .header("content-type", content_type)
            .body(Body::from(body))
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (status, String::from_utf8_lossy(&bytes).to_string())
    }

    // These used to panic the request task instead of answering the client.
    #[tokio::test]
    async fn rejects_a_malformed_multipart_body() {
        let (status, body) = post_body(
            "multipart/form-data; boundary=abc",
            "this is not a multipart body",
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body.starts_with("Malformed multipart body:"), "{}", body);
    }

    #[tokio::test]
    async fn rejects_a_body_that_is_not_multipart_at_all() {
        let (status, _) = post_body("application/json", "{}").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn accepts_a_part_without_a_name() {
        let body = "--abc\r\nContent-Disposition: form-data\r\n\r\nstray\r\n--abc--\r\n";
        let (status, response) = post_body("multipart/form-data; boundary=abc", body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(response, "No template provided");
    }

    fn compile_failure(main: &str, extra: &[(&str, &str)]) -> String {
        let main_source = Source::detached(main.to_string());
        let extra_sources: Vec<Source> = extra
            .iter()
            .map(|(name, text)| {
                Source::new(FileId::new(None, VirtualPath::new(name)), text.to_string())
            })
            .collect();
        let sources: HashMap<FileId, Source> = std::iter::once(main_source.clone())
            .chain(extra_sources.iter().cloned())
            .map(|source| (source.id(), source))
            .collect();
        let engine = TypstEngine::builder()
            .main_file(main_source)
            .with_static_source_file_resolver(extra_sources)
            .build();
        let output = engine.compile::<PagedDocument>().output;
        match output {
            Ok(_) => panic!("expected the template to fail to compile"),
            Err(TypstAsLibError::TypstSource(diagnostics)) => {
                format_diagnostics(&diagnostics, &sources)
            }
            Err(e) => panic!("expected a source diagnostic, got {:?}", e),
        }
    }

    // The failure that motivated this: a decimal comma inside sqrt() makes
    // typst read one argument as two, and the span alone said nothing useful.
    #[test]
    fn reports_line_and_column_for_a_compile_error() {
        let report = compile_failure("#set page(width: 100pt)\n$sqrt(0,5)$\n", &[]);
        assert!(report.starts_with("error: unexpected argument"), "{}", report);
        assert!(report.contains("--> main.typ:2:9"), "{}", report);
        assert!(report.contains("$sqrt(0,5)$"), "{}", report);
    }

    #[test]
    fn reports_the_file_an_error_came_from() {
        let report = compile_failure(
            "#import \"lib.typ\": *\n#broken()\n",
            &[("lib.typ", "#let broken() = $sqrt(0,5)$\n")],
        );
        assert!(report.starts_with("error: unexpected argument"), "{}", report);
        assert!(report.contains("--> lib.typ:1:25"), "{}", report);
        assert!(
            report.contains("call of function `broken` at main.typ:2:2"),
            "{}",
            report
        );
    }

    #[test]
    fn truncates_a_long_line_around_the_offending_column() {
        let padding = "x".repeat(400);
        let line = format!("{}0,5{}", padding, padding);
        let excerpt = excerpt_around(&line, 400);
        assert!(excerpt.chars().count() <= MAX_EXCERPT_CHARS + 2, "{}", excerpt);
        assert!(excerpt.starts_with('…') && excerpt.ends_with('…'), "{}", excerpt);
        assert!(excerpt.contains("0,5"), "{}", excerpt);
    }

    #[test]
    fn keeps_a_short_line_intact() {
        assert_eq!(excerpt_around("$sqrt(0,5)$", 6), "$sqrt(0,5)$");
    }
}
