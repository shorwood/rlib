#![allow(dead_code, unknown_lints)]

#[derive(strum::EnumString, strum::VariantNames)]
enum State {
    #[strum(serialize = "queued", to_string = "pending")]
    Pending,
    Complete,
}

fn main() {}
