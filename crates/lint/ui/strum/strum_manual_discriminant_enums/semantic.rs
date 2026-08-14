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

enum OwnedEvent {
    Created(String),
    Deleted(u64),
}

enum OwnedEventKind {
    Created,
    Deleted,
}

impl From<OwnedEvent> for OwnedEventKind {
    fn from(event: OwnedEvent) -> Self {
        match event {
            OwnedEvent::Created(_) => Self::Created,
            OwnedEvent::Deleted(_) => Self::Deleted,
        }
    }
}

mod private_api {
    pub enum Event {
        Created(String),
        Deleted(u64),
    }

    pub enum EventKind {
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
}

enum PublicSource {
    Created(String),
    Deleted(u64),
}

pub enum PublicKind {
    Created,
    Deleted,
}

impl From<&PublicSource> for PublicKind {
    fn from(event: &PublicSource) -> Self {
        match event {
            PublicSource::Created(_) => Self::Created,
            PublicSource::Deleted(_) => Self::Deleted,
        }
    }
}

enum SchemaSource {
    Created(String),
    Deleted(u64),
}

#[derive(serde::Serialize)]
enum SchemaKind {
    Created,
    Deleted,
}

impl From<&SchemaSource> for SchemaKind {
    fn from(event: &SchemaSource) -> Self {
        match event {
            SchemaSource::Created(_) => Self::Created,
            SchemaSource::Deleted(_) => Self::Deleted,
        }
    }
}

fn main() {}
