#![allow(dead_code, serde_defaults_hiding_missing_data, unknown_lints)]

#[derive(serde::Serialize, serde::Deserialize)]
struct Profile {
    #[serde(skip_serializing_if = "String::is_empty")]
    display_name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    biography: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    avatar: Option<String>,
}

fn main() {}
