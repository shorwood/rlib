//! Public lint documentation contract tests.

use std::fs;
use std::path::{Path, PathBuf};

const EXPECTED_HEADINGS: [&str; 4] = [
    "## What it does",
    "## Why is this bad?",
    "## Example",
    "## Use instead",
];

/// Requires every active lint to use the public directory and documentation convention.
#[test]
fn active_lints_have_canonical_public_documentation() {
    let rules = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/rules");
    let mut lint_directories = Vec::new();
    collect_lint_directories(&rules.join("core"), &mut lint_directories);
    #[cfg(feature = "bon")]
    collect_lint_directories(&rules.join("bon"), &mut lint_directories);
    #[cfg(feature = "framework")]
    collect_lint_directories(&rules.join("framework"), &mut lint_directories);
    #[cfg(feature = "derive_more")]
    collect_lint_directories(&rules.join("derive_more"), &mut lint_directories);
    #[cfg(feature = "miette")]
    collect_lint_directories(&rules.join("miette"), &mut lint_directories);
    #[cfg(feature = "serde")]
    collect_lint_directories(&rules.join("serde"), &mut lint_directories);
    #[cfg(feature = "strum")]
    collect_lint_directories(&rules.join("strum"), &mut lint_directories);
    #[cfg(feature = "thiserror")]
    collect_lint_directories(&rules.join("thiserror"), &mut lint_directories);
    #[cfg(feature = "leptos")]
    collect_lint_directories(&rules.join("leptos"), &mut lint_directories);

    let expected = 60
        + 14 * usize::from(cfg!(feature = "bon"))
        + 26 * usize::from(cfg!(feature = "leptos"))
        + 20 * usize::from(cfg!(feature = "derive_more"))
        + usize::from(cfg!(feature = "framework"))
        + 9 * usize::from(cfg!(feature = "miette"))
        + 16 * usize::from(cfg!(feature = "serde"))
        + 19 * usize::from(cfg!(feature = "strum"))
        + 11 * usize::from(cfg!(feature = "thiserror"));
    assert_eq!(
        lint_directories.len(),
        expected,
        "update the documented active-lint count"
    );
    for directory in lint_directories {
        verify_lint_documentation(&directory);
    }
}

/// Collects direct child directories whose module declares a Dylint lint.
fn collect_lint_directories(parent: &Path, lint_directories: &mut Vec<PathBuf>) {
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
        if source.contains("dylint_linting::impl_") {
            lint_directories.push(path);
        }
    }
}

/// Verifies one lint's canonical README and source inclusion contract.
fn verify_lint_documentation(directory: &Path) {
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
        headings, EXPECTED_HEADINGS,
        "{name} uses a noncanonical heading structure"
    );

    let example = section(&readme, EXPECTED_HEADINGS[2], Some(EXPECTED_HEADINGS[3]));
    let replacement = section(&readme, EXPECTED_HEADINGS[3], None);
    assert!(
        example.contains("```rust"),
        "{name} needs a Rust warning example"
    );
    assert!(
        replacement.contains("```rust"),
        "{name} needs a Rust replacement example"
    );
}

/// Extracts the content between one required heading and the next heading.
fn section<'document>(
    document: &'document str,
    heading: &str,
    next_heading: Option<&str>,
) -> &'document str {
    let start = document
        .find(heading)
        .expect("required heading should be present")
        + heading.len();
    let remainder = &document[start..];
    let end = next_heading
        .and_then(|next| remainder.find(next))
        .unwrap_or(remainder.len());
    &remainder[..end]
}
