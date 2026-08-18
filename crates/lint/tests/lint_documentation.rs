//! Public lint documentation contract tests.

use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};

// -----------------------------------------------------------------------------
// LintDocumentation: Canonical public lint documentation
// -----------------------------------------------------------------------------

/// Required second-level headings in every lint README.
const LINT_DOCUMENTATION_HEADINGS: [&str; 4] = [
    "## What it does",
    "## Why is this bad?",
    "## Example",
    "## Use instead",
];

/// Lints whose behavior is controlled by one or more public configuration keys.
const LINT_DOCUMENTATION_CONFIGURABLE_LINTS: &[&str] = &[
    "bon/bon_required_builder_members_breaking_compatibility",
    "core/deeply_nested_control_flow",
    "core/incoherent_extension_traits",
    "core/long_method_chains",
    "core/missing_code_phase_comments",
    "core/needlessly_nested_control_flow",
    "core/overloaded_declaration_sections",
    "core/oversized_match_arms",
    "derive_more/derive_more_manual_error_impls",
    "derive_more/derive_more_manual_variant_accessors",
    "framework/framework_resolution_required",
    "leptos/leptos_excessive_component_composition_depth",
    "leptos/leptos_excessive_component_props",
    "leptos/leptos_excessively_nested_views",
    "leptos/leptos_fragmented_reactive_state",
    "leptos/leptos_missing_view_attribute_group_comments",
    "leptos/leptos_missing_view_section_comments",
    "leptos/leptos_noncanonical_view_formatting",
    "leptos/leptos_overpopulated_component_modules",
    "leptos/leptos_oversized_event_handlers",
    "leptos/leptos_oversized_reactive_setups",
    "leptos/leptos_oversized_view_attribute_groups",
    "leptos/leptos_oversized_view_sections",
    "leptos/leptos_repeated_view_fragments",
    "leptos/leptos_server_functions_without_authorization_boundaries",
    "leptos_styling/leptos_styling_noncanonical_css",
    "miette/miette_generic_diagnostic_help",
    "strum/strum_manual_enum_iteration",
    "strum/strum_manual_enum_predicates",
    "strum/strum_manual_enum_string_conversions",
    "strum/strum_manual_enum_string_parsers",
    "strum/strum_manual_variant_arrays",
    "thiserror/thiserror_manual_error_impls",
    "thiserror/thiserror_manual_from_error_variants",
];

/// Collects direct child directories whose module declares a Dylint lint.
fn lint_documentation_collect_directories(parent: &Path, lint_directories: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(parent).expect("rules directory should be readable") {
        let path = entry.expect("rule entry should be readable").path();
        if path.extension().is_some_and(|extension| extension == "rs") {
            let source = fs::read_to_string(&path).expect("rule source should be readable");
            assert!(
                !source.contains("crate::impl_"),
                "active lint must live in <lint_name>/mod.rs: {}",
                path.display()
            );
            continue;
        }
        let module = path.join("mod.rs");
        let Ok(source) = fs::read_to_string(&module) else {
            continue;
        };
        if !source.contains("crate::impl_") {
            continue;
        }
        lint_directories.push(path);
    }
}

/// Extracts the content between one required heading and the next heading.
fn lint_documentation_section<'document>(
    document: &'document str,
    bounds: Range<&str>,
) -> &'document str {
    let start = document
        .find(bounds.start)
        .expect("required heading should be present")
        + bounds.start.len();
    let remainder = &document[start..];
    let end = if bounds.end.is_empty() {
        remainder.len()
    } else {
        remainder
            .find(bounds.end)
            .expect("required following heading should be present")
    };
    &remainder[..end]
}

/// Returns whether one required README section contains a Rust example.
fn lint_documentation_section_has_rust_example(document: &str, bounds: Range<&str>) -> bool {
    lint_documentation_section(document, bounds).contains("```rust")
}

/// Verifies one lint's canonical README and source inclusion contract.
fn lint_documentation_verify(directory: &Path) {
    let name = directory
        .file_name()
        .and_then(|name| name.to_str())
        .expect("lint directory should have a UTF-8 name");
    let source = fs::read_to_string(directory.join("mod.rs"))
        .expect("lint implementation should be readable");
    assert!(
        source.contains("#[doc = include_str!(\"README.md\")]"),
        "{name} must include its README as lint documentation"
    );

    let readme = fs::read_to_string(directory.join("README.md"))
        .unwrap_or_else(|_| panic!("{name} must have README.md"));
    assert_eq!(readme.lines().next(), Some(format!("# {name}").as_str()));
    let headings: Vec<_> = readme
        .lines()
        .filter(|line| line.starts_with("## "))
        .collect();
    assert_eq!(
        headings.get(..LINT_DOCUMENTATION_HEADINGS.len()),
        Some(LINT_DOCUMENTATION_HEADINGS.as_slice()),
        "{name} uses a noncanonical heading structure"
    );
    assert!(
        headings.len() == LINT_DOCUMENTATION_HEADINGS.len()
            || headings.as_slice()
                == [
                    LINT_DOCUMENTATION_HEADINGS.as_slice(),
                    &["## Configuration"]
                ]
                .concat(),
        "{name} may only add a final Configuration section"
    );

    assert!(
        lint_documentation_section_has_rust_example(
            &readme,
            LINT_DOCUMENTATION_HEADINGS[2]..LINT_DOCUMENTATION_HEADINGS[3],
        ),
        "{name} needs a Rust warning example"
    );
    assert!(
        lint_documentation_section_has_rust_example(
            &readme,
            LINT_DOCUMENTATION_HEADINGS[3]
                ..headings
                    .get(LINT_DOCUMENTATION_HEADINGS.len())
                    .copied()
                    .unwrap_or(""),
        ),
        "{name} needs a Rust replacement example"
    );
}

/// Requires every active lint to use the public directory and documentation convention.
#[test]
fn lint_documentation_is_canonical_for_active_lints() {
    let rules = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/rules");
    let mut lint_directories = Vec::new();
    lint_documentation_collect_directories(&rules.join("core"), &mut lint_directories);
    #[cfg(feature = "bon")]
    lint_documentation_collect_directories(&rules.join("bon"), &mut lint_directories);
    #[cfg(feature = "framework")]
    lint_documentation_collect_directories(&rules.join("framework"), &mut lint_directories);
    #[cfg(feature = "derive_more")]
    lint_documentation_collect_directories(&rules.join("derive_more"), &mut lint_directories);
    #[cfg(feature = "miette")]
    lint_documentation_collect_directories(&rules.join("miette"), &mut lint_directories);
    #[cfg(feature = "serde")]
    lint_documentation_collect_directories(&rules.join("serde"), &mut lint_directories);
    #[cfg(feature = "strum")]
    lint_documentation_collect_directories(&rules.join("strum"), &mut lint_directories);
    #[cfg(feature = "thiserror")]
    lint_documentation_collect_directories(&rules.join("thiserror"), &mut lint_directories);
    #[cfg(feature = "leptos")]
    lint_documentation_collect_directories(&rules.join("leptos"), &mut lint_directories);
    #[cfg(feature = "leptos_styling")]
    lint_documentation_collect_directories(&rules.join("leptos_styling"), &mut lint_directories);

    let expected = 65
        + 14 * usize::from(cfg!(feature = "bon"))
        + 37 * usize::from(cfg!(feature = "leptos"))
        + 6 * usize::from(cfg!(feature = "leptos_styling"))
        + 20 * usize::from(cfg!(feature = "derive_more"))
        + usize::from(cfg!(feature = "framework"))
        + 15 * usize::from(cfg!(feature = "miette"))
        + 16 * usize::from(cfg!(feature = "serde"))
        + 19 * usize::from(cfg!(feature = "strum"))
        + 11 * usize::from(cfg!(feature = "thiserror"));
    assert_eq!(
        lint_directories.len(),
        expected,
        "update the documented active-lint count"
    );
    for directory in lint_directories {
        lint_documentation_verify(&directory);
    }
}

/// Keeps configuration discoverable from every lint whose outcome it changes.
#[test]
fn lint_documentation_configurable_lints_document_their_keys() {
    let rules = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/rules");
    for lint in LINT_DOCUMENTATION_CONFIGURABLE_LINTS {
        let readme = fs::read_to_string(rules.join(lint).join("README.md"))
            .unwrap_or_else(|_| panic!("{lint} must have README.md"));
        assert!(
            readme.contains("\n## Configuration\n"),
            "{lint} must document its configuration"
        );
    }
}
