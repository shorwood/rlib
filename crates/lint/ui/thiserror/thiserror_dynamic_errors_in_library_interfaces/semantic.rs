#![allow(dead_code, unknown_lints)]

use std::error::Error;

#[derive(Debug, thiserror::Error)]
#[error("load failed")]
pub struct LoadError;

pub fn erased_load() -> Result<(), Box<dyn Error>> {
    Ok(())
}

pub fn typed_load() -> Result<(), LoadError> {
    Ok(())
}

fn private_load() -> Result<(), Box<dyn Error>> {
    Ok(())
}

fn main() {}
