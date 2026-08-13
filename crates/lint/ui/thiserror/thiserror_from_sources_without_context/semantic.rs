#![allow(dead_code, unknown_lints)]

use std::io;

#[derive(Debug, thiserror::Error)]
enum LoadError {
    #[error(transparent)]
    Io(#[from] io::Error),
}

fn read_body() -> Result<(), io::Error> {
    Ok(())
}

fn read_header() -> Result<(), io::Error> {
    Ok(())
}

fn load() -> Result<(), LoadError> {
    read_header()?;
    read_body()?;
    Ok(())
}

fn main() {}
