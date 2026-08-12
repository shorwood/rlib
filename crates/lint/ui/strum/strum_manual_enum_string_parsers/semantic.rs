#![allow(dead_code, unknown_lints)]

use std::str::FromStr;

enum Stage {
    Planned,
    Complete,
}

impl FromStr for Stage {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "Planned" => Ok(Self::Planned),
            "Complete" => Ok(Self::Complete),
            _ => Err(()),
        }
    }
}

fn main() {}
