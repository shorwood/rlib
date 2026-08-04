const CROSS_CUTTING_LINT_ALLOWS: [&str; 16] = [
    "-A",
    "bare_tuple_types",
    "-A",
    "duplicate_section_divider_prefixes",
    "-A",
    "incoherent_type_family_names",
    "-A",
    "malformed_section_dividers",
    "-A",
    "mismatched_section_divider_prefixes",
    "-A",
    "missing_section_dividers",
    "-A",
    "nested_tuple_types",
    "-A",
    "positional_aggregate_fields",
];

/// Runs every default-configuration UI fixture against the lint library.
#[test]
fn ui() {
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), "ui")
        .rustc_flags(CROSS_CUTTING_LINT_ALLOWS)
        .run();
}
