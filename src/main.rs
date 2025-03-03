use axum::{
    Json, Router,
    body::Body,
    http::{Response, StatusCode},
    response::IntoResponse,
    routing::post,
};
use derive_typst_intoval::{IntoDict, IntoValue};
use serde::{Deserialize, Serialize};
use typst::foundations::{Bytes, Dict, IntoValue};
use typst_as_lib::TypstEngine;

static TEMPLATE_FILE: &str = include_str!("./templates/template.typ");
static FONT: &[u8] = include_bytes!("./fonts/texgyrecursor-regular.otf");
static IMAGE: &[u8] = include_bytes!("./templates/images/typst.png");

#[tokio::main]
async fn main() {
    // build our application with a route
    let app = Router::new().route("/", post(create_pdf));

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3009").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn create_pdf(
    // this argument tells axum to parse the request body
    // as JSON into a `CreateUser` type
    Json(payload): Json<CreateUser>,
) -> impl IntoResponse {
    // insert your application logic here
    // let user = User {
    //     id: 1337,
    //     username: payload.username,
    // };
    println!("payload: {:?}", payload);

    // Read in fonts and the main source file.
    // We can use this template more than once, if needed (Possibly
    // with different input each time).
    let template = TypstEngine::builder()
        .main_file(TEMPLATE_FILE)
        .fonts([FONT])
        .build();

    // Run it
    let doc = template
        .compile_with_input(dummy_data())
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
    // this will be converted into a JSON response
    // with a status code of `201 Created`
    // (StatusCode::CREATED, Json(user))
}

// the input to our `create_user` handler
#[derive(Deserialize, Debug)]
struct CreateUser {
    username: String,
}

// the output to our `create_user` handler
#[derive(Serialize, Debug)]
struct User {
    id: u64,
    username: String,
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
