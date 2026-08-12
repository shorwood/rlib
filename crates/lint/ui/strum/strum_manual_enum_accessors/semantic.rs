#![feature(register_tool)]
#![allow(unknown_lints)]
#![allow(
    clippy::match_wildcard_for_single_variants,
    clippy::missing_const_for_fn,
    clippy::unnecessary_wraps,
    dead_code
)]
#![register_tool(rlib_lint)]

enum Message {
    Write(Vec<u8>),
    Quit,
}

impl Message {
    pub fn try_as_write(self) -> Option<Vec<u8>> {
        match self {
            Self::Write(value) => Some(value),
            _ => None,
        }
    }

    pub fn try_as_write_ref(&self) -> Option<&Vec<u8>> {
        match self {
            Self::Write(value) => Some(value),
            _ => None,
        }
    }

    pub fn try_as_write_mut(&mut self) -> Option<&mut Vec<u8>> {
        match self {
            Self::Write(value) => Some(value),
            _ => None,
        }
    }
}

enum Partial {
    Value(u32),
}

impl Partial {
    fn try_as_value(self) -> Option<u32> {
        match self {
            Self::Value(value) => Some(value),
        }
    }
}

fn main() {}
