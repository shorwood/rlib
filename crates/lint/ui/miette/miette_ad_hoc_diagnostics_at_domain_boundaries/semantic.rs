#![allow(dead_code, unknown_lints)]

use miette::WrapErr;

pub struct Loader;

impl Loader {
    pub fn primary(&self) -> miette::Result<()> {
        Err(miette::miette!("loader is unavailable"))
    }

    pub fn fallback(&self) -> miette::Result<()> {
        Err(miette::miette!("loader is unavailable"))
    }
}

pub fn load_primary() -> miette::Result<()> {
    Err(miette::miette!("configuration is missing"))
}

pub fn load_fallback() -> miette::Result<()> {
    Err(miette::miette!("configuration is missing"))
}

pub fn validate_primary() -> miette::Result<()> {
    Err(miette::Report::from(miette::diagnostic!(
        "input is invalid"
    )))
}

pub fn validate_fallback() -> miette::Result<()> {
    Err(miette::Report::from(miette::diagnostic!(
        "input is invalid"
    )))
}

pub fn read_primary() -> miette::Result<()> {
    Err::<(), _>(miette::miette!("primary source is unavailable"))
        .wrap_err("configuration could not be read")
}

pub fn read_fallback() -> miette::Result<()> {
    Err::<(), _>(miette::miette!("fallback source is unavailable"))
        .wrap_err("configuration could not be read")
}

pub fn coded_primary() -> miette::Result<()> {
    Err(miette::miette!(
        code = "configuration::coded",
        "coded configuration failure"
    ))
}

pub fn coded_fallback() -> miette::Result<()> {
    Err(miette::miette!(
        code = "configuration::coded",
        "coded configuration failure"
    ))
}

fn one_off_glue() -> miette::Result<()> {
    Err(miette::miette!("temporary startup failure"))
}

fn dynamic(name: &str) -> miette::Result<()> {
    Err(miette::miette!("configuration {name} is missing"))
}

fn another_dynamic(name: &str) -> miette::Result<()> {
    Err(miette::miette!("configuration {name} is missing"))
}

fn main() {}
