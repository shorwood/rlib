const CROSS_CUTTING_LINT_ALLOWS: [&str; 24] = [
    "-A",
    "bare_tuple_types",
    "-A",
    "bidirectional_module_dependencies",
    "-A",
    "duplicate_section_divider_prefixes",
    "-A",
    "incoherent_type_family_names",
    "-A",
    "implicit_first_wins_deduplication",
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
    "-A",
    "repeated_identical_statements",
    "-A",
    "unparenthesized_mixed_boolean_operators",
];

/// Runs every default-configuration UI fixture against the lint library.
#[test]
fn ui() {
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), "ui")
        .rustc_flags(CROSS_CUTTING_LINT_ALLOWS)
        .run();
}
