#![feature(register_tool)]
#![allow(unknown_lints)]
#![allow(
    clippy::match_like_matches_macro,
    clippy::match_wildcard_for_single_variants,
    clippy::missing_const_for_fn,
    dead_code
)]
#![register_tool(rlib_lint)]

enum Message {
    Write(Vec<u8>),
    Quit,
}

impl Message {
    pub fn is_write(&self) -> bool {
        matches!(self, Self::Write(_))
    }

    pub fn is_quit(&self) -> bool {
        match self {
            Self::Quit => true,
            _ => false,
        }
    }
}

enum Partial {
    Ready,
    Done,
}

impl Partial {
    fn is_ready(&self) -> bool {
        matches!(self, Self::Ready)
    }
}

fn main() {}
