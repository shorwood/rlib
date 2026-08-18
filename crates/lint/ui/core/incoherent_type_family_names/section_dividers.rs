#![allow(dead_code)]
#![warn(rlib::incoherent_type_family_names)]

// -----------------------------------------------------------------------------
// SectionDivider: Family naming fixture
// -----------------------------------------------------------------------------

struct SectionDividerConfig;

struct SectionDividerLibraryConfig {
    section_dividers: SectionDividerConfig,
}

fn main() {}
