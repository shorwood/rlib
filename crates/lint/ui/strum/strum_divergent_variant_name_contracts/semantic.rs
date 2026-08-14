#![allow(dead_code, unknown_lints)]

#[derive(strum::EnumString, strum::VariantNames)]
enum State {
    #[strum(serialize = "queued", to_string = "pending")]
    Pending,
    Complete,
}

#[derive(strum::EnumString, strum::VariantNames)]
enum CapturingState {
    Ready,
    #[strum(default)]
    Unknown(String),
}

#[derive(strum::EnumString, strum::VariantNames)]
#[strum(prefix = "api-")]
enum PrefixedState {
    Ready,
}

#[derive(strum::EnumString, strum::VariantNames)]
enum DisabledAlias {
    Ready,
    #[strum(serialize = "legacy", disabled)]
    Legacy,
}

#[derive(strum::EnumString, strum::VariantNames)]
#[strum(serialize_all = "snake_case")]
enum CoherentState {
    WaitingForInput,
}

fn main() {}
