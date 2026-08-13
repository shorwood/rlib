#![allow(dead_code, unknown_lints)]

#[derive(Debug, thiserror::Error)]
enum LookupError {
    #[error("not found")]
    Missing,
}

fn is_missing(error: &LookupError) -> bool {
    error.to_string() == "not found"
}

fn display(error: &LookupError) -> String {
    error.to_string()
}

fn main() {}
