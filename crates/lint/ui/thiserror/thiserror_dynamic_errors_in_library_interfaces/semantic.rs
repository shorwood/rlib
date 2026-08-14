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

pub struct Loader;

impl Loader {
    pub fn erased_method() -> Result<(), Box<dyn Error + Send + Sync>> {
        Ok(())
    }
}

mod internal {
    use std::error::Error;

    pub fn erased_but_unexported() -> Result<(), Box<dyn Error>> {
        Ok(())
    }
}

pub mod lookalike {
    pub struct Result<T, E>(pub T, pub E);

    pub fn custom_result() -> Result<(), Box<dyn std::error::Error>> {
        unimplemented!()
    }
}

fn private_load() -> Result<(), Box<dyn Error>> {
    Ok(())
}

fn main() {}
