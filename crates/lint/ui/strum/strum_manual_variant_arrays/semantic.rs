#![feature(register_tool)]
#![allow(unknown_lints)]
#![allow(dead_code, misordered_module_declarations)]
#![register_tool(rlib_lint)]

#[derive(Clone, Copy)]
pub enum Stage {
    Planned,
    Running,
    Complete,
}

pub const STAGES: &[Stage] = &[Stage::Planned, Stage::Running, Stage::Complete];

impl Stage {
    const ALL: [Self; 3] = [Self::Planned, Self::Running, Self::Complete];
}

fn stage_array() -> [Stage; 3] {
    [Stage::Planned, Stage::Running, Stage::Complete]
}

#[derive(Clone, Copy, strum::EnumCount)]
enum DisabledStage {
    Active,
    #[strum(disabled)]
    Legacy,
}

const ALL_DISABLED_STAGES: &[DisabledStage] = &[DisabledStage::Active, DisabledStage::Legacy];

const ENABLED_STAGES: &[DisabledStage] = &[DisabledStage::Active];

#[derive(Clone, Copy, strum::EnumCount)]
enum SerializedStage {
    #[strum(serialize = "planned")]
    Planned,
    Complete,
}

const SERIALIZED_STAGES: &[SerializedStage] =
    &[SerializedStage::Planned, SerializedStage::Complete];

const WRONG_ORDER: [Stage; 3] = [Stage::Running, Stage::Planned, Stage::Complete];

fn main() {}
