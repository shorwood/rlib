#![allow(dead_code, unknown_lints)]

#[derive(serde::Serialize)]
struct OutputCollision {
    #[serde(rename = "id")]
    internal_id: u64,
    id: String,
}

#[derive(serde::Deserialize)]
struct InputCollision {
    #[serde(alias = "name")]
    display_name: String,
    name: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum CaseCollision {
    HttpServer,
    HTTPServer,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Distinct {
    first: String,
    second: String,
}

fn main() {}
