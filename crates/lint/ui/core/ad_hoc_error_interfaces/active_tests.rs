// compile-flags: --test

#![warn(rlib::ad_hoc_error_interfaces)]
#![allow(dead_code)]

use std::error::Error;
use std::fmt::{self, Display, Formatter};

#[derive(Debug)]
struct TestError {
    message: String,
}

#[cfg(test)]
impl Display for TestError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

#[cfg(test)]
impl Error for TestError {}

#[cfg(test)]
fn active_use() -> Result<(), TestError> {
    Err(TestError { message: "test".to_owned() })
}
