#![allow(clippy::missing_const_for_fn, dead_code, unknown_lints)]

enum Severity {
    Info,
    Error,
}

impl Severity {
    fn message(&self) -> &'static str {
        match self {
            Self::Info => "informational",
            Self::Error => "operation failed",
        }
    }
}

enum Status {
    Ready,
    Failed,
}

impl Status {
    fn error_message(&self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Failed => "failed",
        }
    }

    fn colorize(&self) -> &'static str {
        match self {
            Self::Ready => "green",
            Self::Failed => "red",
        }
    }
}

#[derive(strum::EnumMessage)]
enum PartiallyAnnotated {
    #[strum(message = "ready")]
    Ready,
    Failed,
}

impl PartiallyAnnotated {
    fn color(&self) -> &'static str {
        match self {
            Self::Ready => "green",
            Self::Failed => "red",
        }
    }
}

#[derive(strum::EnumProperty)]
enum PropertyOwned {
    #[strum(props(color = "green"))]
    Ready,
    #[strum(props(color = "red"))]
    Failed,
}

impl PropertyOwned {
    fn color(&self) -> &'static str {
        match self {
            Self::Ready => "green",
            Self::Failed => "red",
        }
    }
}

enum Localized {
    Ready,
    Failed,
}

impl Localized {
    /// Localization policy intentionally stays in the presentation layer.
    fn message(&self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Failed => "failed",
        }
    }
}

fn main() {}
