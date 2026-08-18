#![allow(dead_code, rlib::serde_defaults_hiding_missing_data, unknown_lints)]

#[derive(serde::Serialize, serde::Deserialize)]
struct Profile {
    #[serde(skip_serializing_if = "String::is_empty")]
    display_name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    biography: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    avatar: Option<String>,
}

fn is_zero(value: &u32) -> bool {
    *value == 0
}

#[derive(serde::Serialize, serde::Deserialize)]
struct DocumentedLoss {
    /// The public score.
    #[serde(skip_serializing_if = "is_zero")]
    score: u32,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct TupleLoss(#[serde(skip_serializing_if = "is_zero")] u32);

#[derive(serde::Serialize, serde::Deserialize)]
enum Event {
    Changed {
        #[serde(skip_serializing_if = "is_zero")]
        score: u32,
    },
}

mod custom {
    #[derive(serde::Serialize, serde::Deserialize)]
    pub(super) enum Option<T> {
        Some(T),
        None,
    }

    impl<T> Option<T> {
        pub(super) const fn is_none(&self) -> bool {
            matches!(self, Self::None)
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct CustomOptionLoss {
    #[serde(skip_serializing_if = "custom::Option::is_none")]
    value: custom::Option<u32>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct SkippedInput {
    #[serde(skip_serializing_if = "is_zero", skip_deserializing)]
    score: u32,
}

fn main() {}
