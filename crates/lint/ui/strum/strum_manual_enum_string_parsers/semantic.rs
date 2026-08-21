#![allow(dead_code, unknown_lints)]

use std::str::FromStr;

enum Stage {
    Planned,
    Complete,
}

impl FromStr for Stage {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "Planned" => Ok(Self::Planned),
            "Complete" => Ok(Self::Complete),
            _ => Err(()),
        }
    }
}

enum RuntimeState {
    Hovered,
    FocusWithin,
}

impl RuntimeState {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "Hovered" => Some(Self::Hovered),
            "FocusWithin" => Some(Self::FocusWithin),
            _ => None,
        }
    }
}

enum PolicyParser {
    Ready,
}

impl PolicyParser {
    /// Compatibility policy keeps this Option-returning parser stable.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "Ready" => Some(Self::Ready),
            _ => None,
        }
    }
}

#[derive(strum::EnumCount)]
enum AliasedStage {
    #[strum(serialize = "planned", serialize = "queued")]
    Planned,
    #[strum(serialize = "complete")]
    Complete,
}

impl FromStr for AliasedStage {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "queued" | "planned" => Result::Ok(Self::Planned),
            "complete" => Result::Ok(Self::Complete),
            _ => Result::Err(()),
        }
    }
}

#[derive(strum::EnumCount)]
enum CaseInsensitiveStage {
    #[strum(serialize = "planned", ascii_case_insensitive)]
    Planned,
    Complete,
}

impl FromStr for CaseInsensitiveStage {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "planned" => Ok(Self::Planned),
            "Complete" => Ok(Self::Complete),
            _ => Err(()),
        }
    }
}

#[derive(strum::EnumCount)]
enum DisabledStage {
    Planned,
    #[strum(disabled)]
    Legacy,
}

impl FromStr for DisabledStage {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "Planned" => Ok(Self::Planned),
            "Legacy" => Ok(Self::Legacy),
            _ => Err(()),
        }
    }
}

fn main() {}
