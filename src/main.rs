use axum::{
    Router,
    body::Body,
    extract::{DefaultBodyLimit, Multipart},
    http::{Response, StatusCode},
    response::IntoResponse,
    routing::post,
};
use serde_json::Value;
use std::collections::HashMap;
use tower_http::limit::RequestBodyLimitLayer;
use typst::foundations::{Bytes, Dict, IntoValue};
use typst_as_lib::TypstEngine;

#[tokio::main]
async fn main() {
    let host = std::env::var("HOST").unwrap_or("0.0.0.0".to_string());
    let port = std::env::var("PORT").unwrap_or("3009".to_string());

    println!("typst-server running on http://{}:{}", host, port);

    let app = Router::new()
        .route("/", post(create_pdf))
        .layer(DefaultBodyLimit::disable())
        .layer(RequestBodyLimitLayer::new(
            250 * 1024 * 1024, /* 250mb */
        ));
    let listener = tokio::net::TcpListener::bind(format!("{}:{}", host, port))
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn create_pdf(mut multipart: Multipart) -> impl IntoResponse {
    let mut template_content = None;
    let mut json_data = None;
    let mut data_map: HashMap<String, Bytes> = HashMap::new();
    let mut fonts: Vec<Bytes> = Vec::new();

    while let Some(field) = multipart.next_field().await.unwrap() {
        let name = field.name().unwrap().to_string();
        let file_name = field.file_name().unwrap_or("").to_string();
        // let content_type = field.content_type().unwrap_or("").to_string();
        let data = field.bytes().await.unwrap(); // Handle as bytes

        if name == "template" {
            template_content = Some(String::from_utf8_lossy(&data).to_string());
        } else if name == "data" {
            match serde_json::from_slice::<Value>(&data) {
                Ok(parsed_data) => json_data = Some(parsed_data),
                Err(e) => {
                    return (StatusCode::BAD_REQUEST, format!("Invalid JSON data: {}", e))
                        .into_response();
                }
            }
        } else if name.starts_with("data:") {
            data_map.insert(name.clone(), Bytes::new(data));
        } else if file_name.ends_with(".otf") {
            fonts.push(Bytes::new(data));
        }
    }

    let template_string = match template_content {
        Some(content) => content,
        None => return (StatusCode::BAD_REQUEST, "No template provided").into_response(),
    };

    let data = match json_data {
        Some(data) => data,
        None => return (StatusCode::BAD_REQUEST, "No data provided").into_response(),
    };

    let typst_data = json_to_typst_value(data, &data_map);

    let template = TypstEngine::builder()
        .main_file(template_string)
        .fonts(fonts)
        .build();

    let doc = template
        .compile_with_input(typst_data)
        .output
        .expect("typst::compile() returned an error!");

    let options = Default::default();
    let pdf = typst_pdf::pdf(&doc, &options).expect("Could not generate pdf.");

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/pdf")
        .body(Body::from(pdf))
        .unwrap()
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
