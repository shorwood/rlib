#![allow(clippy::missing_const_for_fn, dead_code, unknown_lints)]

enum Severity {
    Info,
    Error,
}

impl Severity {
    fn message(&self) -> &'static str {
        match self {
            Self::Info => "informational",
            Self::Error => "operation failed",
        }
    }
}

fn main() {}
