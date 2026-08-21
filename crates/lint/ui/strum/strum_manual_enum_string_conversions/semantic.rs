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

#[derive(strum::EnumCount)]
#[strum(serialize_all = "snake_case")]
enum RenamedStage {
    InProgress,
    Complete,
}

impl RenamedStage {
    fn as_static_str(&self) -> &'static str {
        match self {
            Self::InProgress => "in_progress",
            Self::Complete => "complete",
        }
    }
}

enum RuntimeState {
    Hovered,
    FocusWithin,
}

impl RuntimeState {
    pub fn suffix(&self) -> &'static str {
        match self {
            Self::Hovered => "Hovered",
            Self::FocusWithin => "FocusWithin",
        }
    }
}

#[derive(strum::AsRefStr)]
enum ViewportKind {
    Desktop,
    Mobile,
}

impl ViewportKind {
    pub fn name(&self) -> &str {
        self.as_ref()
    }
}

enum DisplayStage {
    Planned,
    Complete,
}

impl std::fmt::Display for DisplayStage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Planned => formatter.write_str("Planned"),
            Self::Complete => formatter.write_str("Complete"),
        }
    }
}

#[derive(strum::EnumCount)]
enum DisabledStage {
    Planned,
    #[strum(disabled)]
    Legacy,
}

impl DisabledStage {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Planned => "Planned",
            Self::Legacy => "Legacy",
        }
    }
}

enum AssociatedStage {
    Planned,
    Complete,
}

impl AssociatedStage {
    fn as_str(stage: &Self) -> &'static str {
        match stage {
            Self::Planned => "Planned",
            Self::Complete => "Complete",
        }
    }
}

enum PolicyStage {
    Planned,
    Complete,
}

impl PolicyStage {
    /// Compatibility policy intentionally keeps these historical names.
    fn name(&self) -> &'static str {
        match self {
            Self::Planned => "first",
            Self::Complete => "last",
        }
    }
}

#[derive(strum::AsRefStr)]
enum BinaryViewportKind {
    Desktop,
    Mobile,
}

impl AsRef<[u8]> for BinaryViewportKind {
    fn as_ref(&self) -> &[u8] {
        b"viewport"
    }
}

impl BinaryViewportKind {
    pub fn bytes(&self) -> &[u8] {
        // False-positive boundary: this forwards to `AsRef<[u8]>`, not generated `AsRef<str>`.
        <Self as AsRef<[u8]>>::as_ref(self)
    }
}

fn main() {}
