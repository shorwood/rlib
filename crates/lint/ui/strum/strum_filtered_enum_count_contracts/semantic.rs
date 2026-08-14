#![allow(clippy::missing_const_for_fn, dead_code, unknown_lints)]

use strum::EnumCount;

#[derive(strum::EnumCount)]
enum Capability {
    Read,
    Write,
    Unknown,
}

fn supported_count() -> usize {
    Capability::COUNT
}

#[derive(strum::EnumCount)]
enum DeprecatedCapability {
    Read,
    #[deprecated]
    Legacy,
}

fn available_count() -> usize {
    DeprecatedCapability::COUNT
}

#[derive(strum::EnumCount)]
enum DisabledCapability {
    Read,
    #[strum(disabled)]
    Legacy,
}

fn enabled_count() -> usize {
    DisabledCapability::COUNT
}

fn enabledness_count() -> usize {
    Capability::COUNT
}

struct LocalCount;

impl LocalCount {
    const COUNT: usize = 1;
}

fn visible_count() -> usize {
    LocalCount::COUNT
}

fn main() {}
