#![allow(dead_code, unknown_lints)]

pub fn public_report() -> miette::Result<()> {
    Ok(())
}

pub fn boxed_report() -> Result<(), Box<miette::Report>> {
    Ok(())
}

pub async fn async_report() -> miette::Result<()> {
    Ok(())
}

pub struct Service;

impl Service {
    pub fn load(&self) -> miette::Result<()> {
        Ok(())
    }
}

pub trait PublicService {
    fn check(&self) -> miette::Result<()>;
}

mod private_api {
    pub fn hidden_report() -> miette::Result<()> {
        Ok(())
    }

    pub struct Hidden;

    impl Hidden {
        pub fn report(&self) -> miette::Result<()> {
            Ok(())
        }
    }
}

struct PrivateService;

impl PrivateService {
    pub fn report(&self) -> miette::Result<()> {
        Ok(())
    }
}

fn private_report() -> miette::Result<()> {
    Ok(())
}

fn main() {}
