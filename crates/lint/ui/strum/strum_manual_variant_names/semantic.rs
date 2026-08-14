#![allow(dead_code, unknown_lints)]

enum Stage {
    Planned,
    Complete,
}

impl Stage {
    const NAMES: [&'static str; 2] = ["Planned", "Complete"];

    const WRONG_ORDER: [&'static str; 2] = ["Complete", "Planned"];
}

pub const STAGE_NAMES: &[&str] = &["Planned", "Complete"];

// Merely containing an enum's spelling is not enough to associate a free table with it.
const BACKSTAGE_NAMES: &[&str] = &["Planned", "Complete"];

#[derive(strum::VariantNames)]
#[allow(strum_conflicting_enum_serializations)]
#[strum(serialize_all = "snake_case", prefix = "api-", suffix = "-v1")]
enum WireState {
    HttpReady,
    #[strum(serialize = "done", serialize = "finished")]
    Complete,
    #[strum(to_string = "explicit", serialize = "alias")]
    Explicit,
    #[strum(disabled)]
    Legacy,
}

const WIRE_STATE_NAMES: &[&str] = &[
    "api-http_ready-v1",
    "api-finished-v1",
    "api-explicit-v1",
    "api-legacy-v1",
];

enum Payload {
    Unit,
    Tuple(u8),
    Struct { value: u8 },
}

impl Payload {
    const NAMES: &[&str] = &["Unit", "Tuple", "Struct"];
}

fn main() {}
