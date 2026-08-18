#![allow(dead_code, unknown_lints)]

#[derive(strum::Display, strum::EnumString)]
#[strum(prefix = "state-")]
enum State {
    Pending,
    Complete,
}

#[derive(strum::AsRefStr, strum::EnumString)]
#[strum(suffix = "-value")]
enum ReferencedState {
    Ready,
}

#[derive(strum::Display, strum::EnumString)]
#[allow(unreachable_patterns, rlib::strum_conflicting_enum_serializations)]
enum Collision {
    #[strum(to_string = "same")]
    First,
    #[strum(to_string = "same")]
    Second,
}

#[derive(strum::Display, strum::EnumString)]
#[strum(serialize_all = "snake_case")]
enum AlignedState {
    HttpReady,
    Complete,
}

#[derive(strum::Display)]
enum OutputOnly {
    Ready,
}

#[derive(strum::EnumString)]
enum InputOnly {
    Ready,
}

fn main() {}
