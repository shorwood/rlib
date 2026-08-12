#![allow(dead_code, unknown_lints)]

use strum::IntoEnumIterator;

#[derive(strum::EnumIter)]
enum MigrationPhase {
    Prepare,
    Apply,
    Verify,
}

fn execute_migration() {
    for _phase in MigrationPhase::iter() {}
}

fn main() {}
