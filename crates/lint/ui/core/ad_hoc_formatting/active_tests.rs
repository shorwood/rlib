// compile-flags: --test

#![warn(ad_hoc_formatting)]
#![allow(dead_code)]

use std::fmt::{self, Display, Formatter};

struct TestDisplay(u64);

impl TestDisplay {
    fn to_text(&self) -> String {
        format!("test {}", self.0)
    }
}

#[cfg(test)]
impl Display for TestDisplay {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}
