use axum::Json;
use axum::body::Body;
use axum::http::{Response as HttpResponse, StatusCode};
use axum::response::{IntoResponse, Response};

type AppResponse = Response;

fn free_response(value: String) -> Response {
    value.into_response()
}

fn alias_response() -> AppResponse {
    StatusCode::NO_CONTENT.into_response()
}

fn opaque_response(value: String) -> impl IntoResponse {
    value
}

fn no_store(value: impl IntoResponse) -> Response {
    value.into_response()
}

fn fallible_response() -> Result<Option<Result<String, Response>>, String> {
    Ok(Some(Ok(String::new())))
}

fn fallible_opaque_response() -> Result<Option<impl IntoResponse>, String> {
    Ok(Some(String::new()))
}

struct Events(Vec<String>);

impl Events {
    fn into_response(self, _delay_ms: u64) -> Response {
        Json(self.0).into_response()
    }

    fn into_sse_response(self, _delay_ms: u64) -> impl IntoResponse {
        Json(self.0)
    }
}

trait ResponseFactory {
    fn response(&self) -> Response;

    fn opaque_response(&self) -> impl IntoResponse;
}

struct Factory;

impl ResponseFactory for Factory {
    fn response(&self) -> Response {
        StatusCode::OK.into_response()
    }

    fn opaque_response(&self) -> impl IntoResponse {
        StatusCode::OK
    }
}

macro_rules! local_response_helper {
    () => {
        fn macro_response() -> Response {
            StatusCode::OK.into_response()
        }
    };
}

local_response_helper!();

struct OwnedResponse;

impl IntoResponse for OwnedResponse {
    fn into_response(self) -> Response {
        StatusCode::OK.into_response()
    }
}

async fn async_response() -> Response {
    StatusCode::OK.into_response()
}

async fn async_opaque_response() -> impl IntoResponse {
    StatusCode::OK
}

#[expect(
    unsafe_code,
    reason = "the fixture verifies that unsafe response functions stay outside this API migration"
)]
unsafe fn unsafe_response() -> Response {
    StatusCode::OK.into_response()
}

const fn constant_response() -> Response {
    panic!("not evaluated")
}

#[expect(
    improper_ctypes_definitions,
    reason = "the fixture verifies that foreign response functions stay outside this API migration"
)]
extern "C" fn foreign_response() -> Response {
    StatusCode::OK.into_response()
}

fn top_level_optional_response() -> Option<Response> {
    None
}

fn response_collection() -> Vec<Response> {
    Vec::new()
}

fn fallible_response_collection() -> Result<Vec<Response>, String> {
    Ok(Vec::new())
}

fn ordinary_json() -> Json<String> {
    Json(String::new())
}

fn ordinary_tuple() -> (StatusCode, String) {
    (StatusCode::OK, String::new())
}

fn ordinary_string() -> String {
    String::new()
}

fn custom_body_response() -> HttpResponse<String> {
    HttpResponse::new(String::new())
}

fn closure_response() {
    let _response = || -> Response { StatusCode::OK.into_response() };
}

fn explicit_default_body_response() -> HttpResponse<Body> {
    HttpResponse::new(Body::empty())
}

fn main() {}
