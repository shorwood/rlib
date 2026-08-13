#![allow(dead_code, unknown_lints)]

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("bad")]
#[diagnostic(url("http://localhost/errors/bad"))]
struct Bad;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("good")]
#[diagnostic(url("https://docs.example.com/errors/good"))]
struct Good;

fn main() {}
