#![allow(dead_code, unknown_lints)]

use std::io;

#[derive(Debug, thiserror::Error)]
enum LoadError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Parse(#[from] std::num::ParseIntError),
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

fn already_contextual() -> Result<(), LoadError> {
    Ok(())
}

fn mixed_propagation() -> Result<(), LoadError> {
    read_header()?;
    already_contextual()?;
    Ok(())
}

fn parse_port() -> Result<(), std::num::ParseIntError> {
    "80".parse::<u16>().map(|_| ())
}

fn parse_timeout() -> Result<(), std::num::ParseIntError> {
    "30".parse::<u16>().map(|_| ())
}

fn load_numbers() -> Result<(), LoadError> {
    parse_port()?;
    parse_timeout()?;
    Ok(())
}

fn main() {}
