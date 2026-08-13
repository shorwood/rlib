#![allow(dead_code, unknown_lints)]

#[derive(serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub struct Session {
    pub access_token: String,
    pub user_name: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub struct SafeSession {
    #[serde(skip_serializing)]
    pub access_token: String,
}

fn main() {}
