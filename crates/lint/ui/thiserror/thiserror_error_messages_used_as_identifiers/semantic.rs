#![allow(dead_code, unknown_lints)]

#[derive(Debug, thiserror::Error)]
enum LookupError {
    #[error("not found")]
    Missing,
}

fn is_missing(error: &LookupError) -> bool {
    error.to_string() == "not found"
}

fn is_missing_reversed(error: &LookupError) -> bool {
    "not found" == error.to_string().as_str()
}

fn has_missing_prefix(error: &LookupError) -> bool {
    error.to_string().starts_with("not")
}

fn has_missing_fragment(error: &LookupError) -> bool {
    error.to_string().contains("found")
}

fn compares_domain_data(error: &LookupError, expected: &str) -> bool {
    error.to_string() == expected
}

fn display(error: &LookupError) -> String {
    error.to_string()
}

fn main() {}
