#![allow(dead_code, unknown_lints)]

enum Message {
    Write(Vec<u8>),
    Quit,
}

impl Message {
    fn is_write(&self) -> bool {
        matches!(self, Self::Write(_))
    }

    fn is_quit(&self) -> bool {
        matches!(self, Self::Quit)
    }
}

enum Partial {
    Ready,
    Waiting,
}

impl Partial {
    fn is_ready(&self) -> bool {
        matches!(self, Self::Ready)
    }
}

fn main() {}
