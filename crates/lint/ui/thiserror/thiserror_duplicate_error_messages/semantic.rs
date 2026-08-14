#![allow(dead_code, unknown_lints)]

#[derive(Debug, thiserror::Error)]
enum LookupError {
    #[error("not found")]
    User,
    #[error("not found")]
    Project,
    #[error("permission denied")]
    Permission,
    #[error("invalid {{input}}")]
    InvalidUserInput,
    #[error("invalid {{input}}")]
    InvalidProjectInput,
    #[error("lookup failed for {0}")]
    DynamicUser(String),
    #[error("lookup failed for {0}")]
    DynamicProject(String),
}

fn main() {}
