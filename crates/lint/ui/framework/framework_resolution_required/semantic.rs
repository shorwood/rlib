#![feature(register_tool)]
#![allow(unknown_lints)]
#![allow(clippy::missing_const_for_fn, dead_code)]
#![register_tool(rlib_lint)]

enum Stage {
    Planned,
    Running,
    Complete,
}

impl Stage {
    const ALL: [Self; 3] = [Self::Planned, Self::Running, Self::Complete];
}

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

fn main() {}
