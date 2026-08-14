#![allow(dead_code, unknown_lints)]

use strum::{IntoEnumIterator, VariantArray};

#[derive(strum::EnumIter)]
enum MigrationPhase {
    Prepare,
    Apply,
    Verify,
}

fn execute_migration() {
    for _phase in MigrationPhase::iter() {}
}

#[derive(strum::VariantArray)]
enum MenuEntry {
    Home,
    Settings,
}

fn render_menu() {
    let _entries = MenuEntry::VARIANTS;
}

fn document_menuet_theory() {
    for _phase in MigrationPhase::iter() {}
}

struct LocalIterator;

impl LocalIterator {
    fn iter() {}
}

fn execute_local_iterator() {
    LocalIterator::iter();
}

fn main() {}
