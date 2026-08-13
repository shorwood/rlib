#![allow(dead_code, unknown_lints)]

#[derive(Debug, thiserror::Error)]
#[error("marker")]
struct Marker;

impl miette::Diagnostic for Marker {}

#[derive(Debug, thiserror::Error)]
#[error("coded")]
struct Coded;

impl miette::Diagnostic for Coded {
    fn code<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        Some(Box::new("domain::coded"))
    }
}

#[derive(Debug, thiserror::Error)]
#[error("custom")]
struct Custom;

impl miette::Diagnostic for Custom {
    fn help<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        if std::env::var_os("HELP").is_some() {
            Some(Box::new("dynamic"))
        } else {
            None
        }
    }
}

fn main() {}
