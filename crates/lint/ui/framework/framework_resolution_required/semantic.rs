#![feature(register_tool)]
#![allow(unknown_lints)]
#![allow(clippy::missing_const_for_fn, dead_code)]
#![register_tool(rlib_lint)]

use std::fmt;
use std::str::FromStr;

enum Stage {
    Planned,
    Running,
    Complete,
}

impl Stage {
    const ALL: [Self; 3] = [Self::Planned, Self::Running, Self::Complete];
}

impl fmt::Display for Stage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Planned => formatter.write_str("Planned"),
            Self::Running => formatter.write_str("Running"),
            Self::Complete => formatter.write_str("Complete"),
        }
    }
}

impl FromStr for Stage {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "Planned" => Ok(Self::Planned),
            "Running" => Ok(Self::Running),
            "Complete" => Ok(Self::Complete),
            _ => Err(()),
        }
    }
}

enum Message {
    Write(Vec<u8>),
    Quit,
}

impl Message {
    pub fn is_write(&self) -> bool {
        matches!(self, Self::Write(_))
    }

    pub fn is_quit(&self) -> bool {
        matches!(self, Self::Quit)
    }
}

enum PrivateMessage {
    Write,
    Quit,
}

impl PrivateMessage {
    fn is_write(&self) -> bool {
        matches!(self, Self::Write)
    }

    fn is_quit(&self) -> bool {
        matches!(self, Self::Quit)
    }
}

fn main() {}
