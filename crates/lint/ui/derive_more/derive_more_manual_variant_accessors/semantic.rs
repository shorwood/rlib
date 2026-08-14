#![allow(dead_code, unknown_lints)]

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

enum PrivateFamily {
    First,
    Second,
}

impl PrivateFamily {
    // False-positive boundary: the derive would widen both methods to public visibility.
    fn is_first(&self) -> bool {
        matches!(self, Self::First)
    }

    fn is_second(&self) -> bool {
        matches!(self, Self::Second)
    }
}

enum DocumentedFamily {
    First,
    Second,
}

impl DocumentedFamily {
    /// This predicate is an authored compatibility contract.
    pub fn is_first(&self) -> bool {
        matches!(self, Self::First)
    }

    pub fn is_second(&self) -> bool {
        matches!(self, Self::Second)
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
