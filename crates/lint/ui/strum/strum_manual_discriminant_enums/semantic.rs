#![allow(dead_code, unknown_lints)]

enum Event {
    Created(String),
    Deleted(u64),
}

enum EventKind {
    Created,
    Deleted,
}

impl From<&Event> for EventKind {
    fn from(event: &Event) -> Self {
        match event {
            Event::Created(_) => Self::Created,
            Event::Deleted(_) => Self::Deleted,
        }
    }
}

fn main() {}
