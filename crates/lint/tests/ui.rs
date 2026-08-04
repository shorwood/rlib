const SECTION_DIVIDER_ALLOWS: [&str; 8] = [
    "-A",
    "duplicate_section_divider_prefixes",
    "-A",
    "malformed_section_dividers",
    "-A",
    "mismatched_section_divider_prefixes",
    "-A",
    "missing_section_dividers",
];

/// Runs every default-configuration UI fixture against the lint library.
#[test]
fn ui() {
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), "ui")
        .rustc_flags(SECTION_DIVIDER_ALLOWS)
        .run();
}
