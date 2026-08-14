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
    fn name(&self) -> &'static str {
        match self {
            Self::Planned => "first",
            Self::Complete => "last",
        }
    }
}

fn main() {}
