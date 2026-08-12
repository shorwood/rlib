#![allow(dead_code, unknown_lints, unreachable_patterns)]

#[derive(strum::EnumString)]
enum Command {
    #[strum(serialize = "run")]
    Start,
    #[strum(serialize = "run")]
    Resume,
}

fn main() {}
