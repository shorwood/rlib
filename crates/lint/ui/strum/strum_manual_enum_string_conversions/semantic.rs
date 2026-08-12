#![allow(clippy::missing_const_for_fn, dead_code, unknown_lints)]

enum Stage {
    Planned,
    Complete,
}

impl Stage {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Planned => "Planned",
            Self::Complete => "Complete",
        }
    }
}

fn main() {}
