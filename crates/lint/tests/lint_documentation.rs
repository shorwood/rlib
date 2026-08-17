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

/// Collects direct child directories whose module declares a Dylint lint.
fn lint_documentation_collect_directories(parent: &Path, lint_directories: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(parent).expect("rules directory should be readable") {
        let path = entry.expect("rule entry should be readable").path();
        if path.extension().is_some_and(|extension| extension == "rs") {
            let source = fs::read_to_string(&path).expect("rule source should be readable");
            assert!(
                !source.contains("dylint_linting::impl_"),
                "active lint must live in <lint_name>/mod.rs: {}",
                path.display()
            );
            continue;
        }
        let module = path.join("mod.rs");
        let Ok(source) = fs::read_to_string(&module) else {
            continue;
        };
        if !source.contains("dylint_linting::impl_") {
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
        headings, LINT_DOCUMENTATION_HEADINGS,
        "{name} uses a noncanonical heading structure"
    );

    assert!(
        lint_documentation_section_has_rust_example(
            &readme,
            LINT_DOCUMENTATION_HEADINGS[2]..LINT_DOCUMENTATION_HEADINGS[3],
        ),
        "{name} needs a Rust warning example"
    );
    assert!(
        lint_documentation_section_has_rust_example(&readme, LINT_DOCUMENTATION_HEADINGS[3].."",),
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

    let expected = 63
        + 14 * usize::from(cfg!(feature = "bon"))
        + 26 * usize::from(cfg!(feature = "leptos"))
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
