#![feature(register_tool)]
#![allow(unknown_lints)]
#![allow(clippy::absolute_paths, clippy::use_self)]
#![allow(
    dead_code,
    framework_resolution_required,
    misordered_module_declarations
)]
#![register_tool(rlib_lint)]

#[derive(Clone, Copy)]
enum Stage {
    Planned,
    Running,
    Complete,
}

const STAGES: [Stage; 3] = [Stage::Planned, Stage::Running, Stage::Complete];

fn array_adapter() -> impl Iterator<Item = Stage> {
    [Stage::Planned, Stage::Running, Stage::Complete].into_iter()
}

fn vector_adapter() -> impl Iterator<Item = Stage> {
    vec![Stage::Planned, Stage::Running, Stage::Complete].into_iter()
}

fn forwarded() -> impl Iterator<Item = Stage> {
    STAGES.into_iter()
}

fn borrowed_adapter() -> impl Iterator<Item = &'static Stage> {
    STAGES.iter()
}

fn copied_adapter() -> impl Iterator<Item = Stage> {
    STAGES.iter().copied()
}

fn array_value() -> [Stage; 3] {
    [Stage::Planned, Stage::Running, Stage::Complete]
}

fn wrong_order() -> impl Iterator<Item = Stage> {
    [Stage::Running, Stage::Planned, Stage::Complete].into_iter()
}

fn contextual(_reverse: bool) -> impl Iterator<Item = Stage> {
    STAGES.into_iter()
}

impl IntoIterator for Stage {
    type Item = Stage;
    type IntoIter = std::array::IntoIter<Stage, 3>;

    fn into_iter(self) -> Self::IntoIter {
        [Stage::Planned, Stage::Running, Stage::Complete].into_iter()
    }
}

fn main() {}
