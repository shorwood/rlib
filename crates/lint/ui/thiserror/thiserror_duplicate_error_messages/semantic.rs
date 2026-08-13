#![allow(dead_code, unknown_lints)]

#[derive(Debug, thiserror::Error)]
enum LookupError {
    #[error("not found")]
    User,
    #[error("not found")]
    Project,
    #[error("permission denied")]
    Permission,
}

fn main() {}
