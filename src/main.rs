use axum::{
    Router,
    body::Body,
    extract::{DefaultBodyLimit, Multipart},
    http::{Response, StatusCode},
    response::IntoResponse,
    routing::post,
};
use serde_json::Value;
use tower_http::limit::RequestBodyLimitLayer;
use typst::foundations::{Dict, IntoValue};
use typst_as_lib::TypstEngine;

static FONT: &[u8] = include_bytes!("./fonts/texgyrecursor-regular.otf");

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", post(create_pdf))
        .layer(DefaultBodyLimit::disable())
        .layer(RequestBodyLimitLayer::new(
            250 * 1024 * 1024, /* 250mb */
        ));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3009").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn create_pdf(mut multipart: Multipart) -> impl IntoResponse {
    let mut template_content = None;
    let mut json_data = None;

    while let Some(field) = multipart.next_field().await.unwrap() {
        let name = field.name().unwrap().to_string();
        let data = field.text().await.unwrap();

        if name == "template" {
            template_content = Some(data);
        } else if name == "data" {
            // Parse the JSON data
            match serde_json::from_str::<Value>(&data) {
                Ok(parsed_data) => json_data = Some(parsed_data),
                Err(e) => return (StatusCode::BAD_REQUEST, format!("Invalid JSON data: {}", e)).into_response(),
            }
        }
    }

    // Check if we received a template
    let template_string = match template_content {
        Some(content) => content,
        None => return (StatusCode::BAD_REQUEST, "No template provided").into_response(),
    };

    // Check if we received data
    let data = match json_data {
        Some(data) => data,
        None => return (StatusCode::BAD_REQUEST, "No data provided").into_response(),
    };

    // Convert serde_json::Value to typst::Dict
    let typst_data = json_to_typst_value(data);

    // Build the template with the received content
    let template = TypstEngine::builder()
        .main_file(template_string)
        .fonts([FONT])
        .build();

    // Run it with the JSON data
    let doc = template
        .compile_with_input(typst_data)
        .output
        .expect("typst::compile() returned an error!");

    // Create pdf
    let options = Default::default();
    let pdf = typst_pdf::pdf(&doc, &options).expect("Could not generate pdf.");

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/pdf")
        .body(Body::from(pdf))
        .unwrap()
}

// Convert serde_json::Value to typst::Value
fn json_to_typst_value(value: Value) -> Dict {
    match value {
        Value::Object(map) => {
            let mut dict = Dict::new();
            for (k, v) in map {
                dict.insert(k.into(), json_value_to_typst_value(v));
            }
            dict
        },
        _ => {
            // If the root isn't an object, create a dict with a single value
            let mut dict = Dict::new();
            dict.insert("data".into(), json_value_to_typst_value(value));
            dict
        }
    }
}

fn json_value_to_typst_value(value: Value) -> typst::foundations::Value {
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
        },
        Value::String(s) => s.into_value(),
        Value::Array(arr) => {
            let mut typst_arr = Array::new();
            for item in arr {
                typst_arr.push(json_value_to_typst_value(item));
            }
            TypstValue::Array(typst_arr)
        },
        Value::Object(map) => {
            let mut dict = Dict::new();
            for (k, v) in map {
                dict.insert(k.into(), json_value_to_typst_value(v));
            }
            TypstValue::Dict(dict)
        }
    }
}