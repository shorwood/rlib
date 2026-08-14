#![allow(dead_code, unknown_lints, unreachable_patterns)]

#[derive(strum::EnumString)]
enum Command {
    #[strum(serialize = "run")]
    Start,
    #[strum(serialize = "run")]
    Resume,
}

#[derive(strum::EnumString)]
enum CaseFoldedCommand {
    #[strum(serialize = "gray", ascii_case_insensitive)]
    Gray,
    #[strum(serialize = "GRAY")]
    LegacyGray,
}

#[derive(strum::Display)]
enum ImplicitOutput {
    #[strum(serialize = "go", serialize = "proceed")]
    Start,
}

#[derive(strum::Display)]
enum ExplicitOutput {
    #[strum(serialize = "go", serialize = "proceed", to_string = "go")]
    Start,
}

#[derive(strum::EnumString)]
enum DisabledCollision {
    #[strum(serialize = "same")]
    Active,
    #[strum(serialize = "same", disabled)]
    Disabled,
}

fn main() {}
