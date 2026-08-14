#![allow(dead_code, non_snake_case, unknown_lints)]

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

#[derive(serde::Serialize, serde::Deserialize)]
enum VariantFieldCollision {
    Updated {
        #[serde(rename = "id")]
        internal_id: u64,
        id: String,
    },
}

#[derive(serde::Serialize)]
#[serde(rename_all_fields = "snake_case")]
enum VariantFieldCaseCollision {
    Started { HttpServer: u16, HTTPServer: u16 },
}

#[derive(serde::Serialize)]
struct MultipleOutputCollisions {
    #[serde(rename = "b")]
    first_b: u8,
    b: u8,
    #[serde(rename = "a")]
    first_a: u8,
    a: u8,
}

fn main() {}
