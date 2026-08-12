#![allow(dead_code, unknown_lints)]

#[derive(strum::EnumIter)]
enum Job {
    Pending(String),
    Complete,
}

fn main() {}
