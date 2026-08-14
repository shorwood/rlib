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

enum PairMessage {
    Values(u8, u16),
    Quit,
}

impl PairMessage {
    pub fn try_as_values(self) -> Option<(u8, u16)> {
        match self {
            Self::Values(left, right) => Some((left, right)),
            _ => None,
        }
    }

    pub fn try_as_values_ref(&self) -> Option<(&u8, &u16)> {
        match self {
            Self::Values(left, right) => Some((left, right)),
            _ => None,
        }
    }

    pub fn try_as_values_mut(&mut self) -> Option<(&mut u8, &mut u16)> {
        match self {
            Self::Values(left, right) => Some((left, right)),
            _ => None,
        }
    }
}

enum PrivateMessage {
    Value(u32),
    Quit,
}

impl PrivateMessage {
    fn try_as_value(self) -> Option<u32> {
        match self {
            Self::Value(value) => Some(value),
            _ => None,
        }
    }

    fn try_as_value_ref(&self) -> Option<&u32> {
        match self {
            Self::Value(value) => Some(value),
            _ => None,
        }
    }

    fn try_as_value_mut(&mut self) -> Option<&mut u32> {
        match self {
            Self::Value(value) => Some(value),
            _ => None,
        }
    }
}

fn main() {}
