#![allow(dead_code, serde_unstable_implicit_wire_names, unknown_lints)]

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

#[derive(serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialEvent {
    Created {
        primary_access_token_value: String,
        encrypted_refresh_token: String,
    },
}

#[derive(serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub struct RawIdentifier {
    pub r#api_key: Vec<u8>,
}

mod private_api {
    #[derive(serde::Serialize)]
    #[serde(rename_all = "snake_case")]
    pub struct InternalSession {
        pub access_token: String,
    }
}

pub mod custom {
    #[derive(serde::Serialize)]
    pub struct String(pub u64);
}

#[derive(serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub struct CustomCarrier {
    pub access_token: custom::String,
}

fn main() {}
