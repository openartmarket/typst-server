use axum::{
    Router,
    body::Body,
    extract::{DefaultBodyLimit, Multipart},
    http::{Response, StatusCode},
    response::IntoResponse,
    routing::post,
};
use derive_typst_intoval::{IntoDict, IntoValue};
use serde::Deserialize;
use tower_http::limit::RequestBodyLimitLayer;
use typst::foundations::{Bytes, Dict, IntoValue};
use typst_as_lib::TypstEngine;

// static TEMPLATE_FILE: &str = include_str!("./templates/template.typ");
static FONT: &[u8] = include_bytes!("./fonts/texgyrecursor-regular.otf");
static IMAGE: &[u8] = include_bytes!("./templates/images/typst.png");

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

    while let Some(field) = multipart.next_field().await.unwrap() {
        let name = field.name().unwrap().to_string();
        let data = field.text().await.unwrap();

        // Store the template content if the field name is "template"
        if name == "template" {
            template_content = Some(data);
        } else if name == "data" {
            // Optionally parse JSON data if you're sending that too
            // input_data = Some(serde_json::from_str(&data).unwrap());
        }
    }

    // Check if we received a template
    let template_string = match template_content {
        Some(content) => content,
        None => return (StatusCode::BAD_REQUEST, "No template provided").into_response(),
    };

    // Build the template with the received content
    let template = TypstEngine::builder()
        .main_file(template_string) // Use main_content instead of main_file
        .fonts([FONT])
        .build();

    // Run it
    let doc = template
        .compile_with_input(dummy_data()) // or use input_data if you parsed it
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

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct Input {
    template: String,
}

#[derive(Debug, Clone, IntoValue, IntoDict)]
struct Content {
    v: Vec<ContentElement>,
}

// Implement Into<Dict> manually, so we can just pass the struct
// to the compile function.
impl From<Content> for Dict {
    fn from(value: Content) -> Self {
        value.into_dict()
    }
}

#[derive(Debug, Clone, Default, IntoValue, IntoDict)]
struct ContentElement {
    heading: String,
    text: Option<String>,
    num1: i32,
    num2: Option<i32>,
    image: Option<Bytes>,
}

fn dummy_data() -> Content {
    Content {
        v: vec![
            ContentElement {
                heading: "Foo".to_owned(),
                text: Some("Hello World!".to_owned()),
                num1: 1,
                num2: Some(42),
                image: Some(Bytes::new(IMAGE.to_vec())),
            },
            ContentElement {
                heading: "Bar".to_owned(),
                num1: 2,
                ..Default::default()
            },
        ],
    }
}
