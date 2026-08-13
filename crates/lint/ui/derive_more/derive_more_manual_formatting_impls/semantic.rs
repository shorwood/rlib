#![allow(dead_code, misordered_module_declarations, unknown_lints)]

use std::fmt::{self, Display, Formatter, LowerHex};

struct UserId(u64);

impl Display for UserId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

impl LowerHex for UserId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        LowerHex::fmt(&self.0, formatter)
    }
}

struct Redacted(String);

impl Display for Redacted {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "[redacted:{}]", self.0.len())
    }
}

fn main() {}
