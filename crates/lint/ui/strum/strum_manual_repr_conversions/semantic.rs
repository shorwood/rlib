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

#[repr(i16)]
enum SparseStatus {
    Ready = 5,
    Done = 9,
}

impl SparseStatus {
    fn from_repr(value: i16) -> Option<Self> {
        match value {
            5 => Option::Some(Self::Ready),
            9 => Option::Some(Self::Done),
            _ => Option::None,
        }
    }
}

#[repr(u8)]
enum ImplicitStatus {
    Ready,
    Done,
}

impl ImplicitStatus {
    fn from_repr(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Ready),
            1 => Some(Self::Done),
            _ => None,
        }
    }
}

#[repr(u8)]
enum SwappedStatus {
    Ready = 1,
    Done = 2,
}

impl SwappedStatus {
    fn from_repr(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Done),
            2 => Some(Self::Ready),
            _ => None,
        }
    }
}

#[repr(u8)]
enum WideInputStatus {
    Ready = 1,
    Done = 2,
}

impl WideInputStatus {
    fn from_repr(value: u16) -> Option<Self> {
        match value {
            1 => Some(Self::Ready),
            2 => Some(Self::Done),
            _ => None,
        }
    }
}

#[derive(strum::EnumCount)]
#[repr(u8)]
enum SerializedStatus {
    #[strum(serialize = "ready")]
    Ready = 1,
    Done = 2,
}

impl SerializedStatus {
    fn from_repr(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Ready),
            2 => Some(Self::Done),
            _ => None,
        }
    }
}

fn main() {}
