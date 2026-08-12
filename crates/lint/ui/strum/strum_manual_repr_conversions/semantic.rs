#![feature(register_tool)]
#![allow(unknown_lints)]
#![allow(dead_code)]
#![register_tool(rlib_lint)]

#[repr(u8)]
enum Status {
    Ready = 1,
    Done = 2,
}

impl Status {
    pub const fn from_repr(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Ready),
            2 => Some(Self::Done),
            _ => None,
        }
    }
}

#[repr(u8)]
enum Filtered {
    Ready = 1,
    Done = 2,
}

impl Filtered {
    const fn from_repr(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Ready),
            _ => None,
        }
    }
}

fn main() {}
