//! Manifest-driven UI regression tests.

#![cfg(all(
    feature = "bon",
    feature = "derive_more",
    feature = "leptos_i18n",
    feature = "leptos_styling",
    feature = "miette",
    feature = "serde",
    feature = "strum",
    feature = "thiserror"
))]
#![feature(register_tool)]
#![register_tool(rlib)]
// This private procedural harness mirrors external manifests and intentionally keeps orchestration
// helpers free-standing. Product code remains subject to these policies in the Dylint self-check.
#![allow(
    rlib::constructor_like_free_functions,
    rlib::method_like_free_functions,
    rlib::missing_section_dividers,
    rlib::undocumented_items
)]

use std::collections::BTreeSet;
use std::env::{VarError, current_exe, join_paths, split_paths, var, var_os};
use std::fs::read_dir;
use std::iter::once;
use std::path::Path;
use std::process::Command;

use dylint_testing::ui::Test;
use serde::Deserialize;

const CHILD_PROCESS: &str = "RLIB_LINT_UI_CHILD";

const FIXTURE_FILTER: &str = "RLIB_LINT_UI_FIXTURE";

const PACKAGE: &str = env!("CARGO_PKG_NAME");

const UI_MANIFEST: &str = include_str!("../ui.toml");

// -----------------------------------------------------------------------------
// Manifest: Authoritative UI inventory and shared policy
// -----------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Manifest {
    rustc_flags: Vec<String>,
    core_rustc_flags: Vec<String>,
    core_lints: Vec<String>,
    core: Vec<String>,
    example: Vec<Example>,
}

// -----------------------------------------------------------------------------
// Example: Per-Cargo-target lint policy
// -----------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Example {
    name: String,
    lints: Vec<String>,
    rustc_flags: Option<Vec<String>>,
    dylint_toml: Option<String>,
}

// -----------------------------------------------------------------------------
// CargoManifest: Cargo example inventory
// -----------------------------------------------------------------------------

#[derive(Deserialize)]
struct CargoManifest {
    example: Vec<CargoExample>,
}

// -----------------------------------------------------------------------------
// CargoExample: Cargo target identity
// -----------------------------------------------------------------------------

#[derive(Deserialize)]
struct CargoExample {
    name: String,
}

fn manifest() -> Manifest {
    toml::from_str(UI_MANIFEST).expect("ui.toml should be valid")
}

fn selected_fixture() -> Option<String> {
    match var(FIXTURE_FILTER) {
        Ok(fixture) if !fixture.is_empty() => Some(fixture),
        Ok(_) | Err(VarError::NotPresent) => None,
        Err(VarError::NotUnicode(_)) => panic!("{FIXTURE_FILTER} must be valid Unicode"),
    }
}

fn unique_names<'a>(kind: &str, names: impl Iterator<Item = &'a str>) -> BTreeSet<String> {
    let names = names.map(str::to_owned).collect::<Vec<_>>();
    let unique = names.iter().cloned().collect::<BTreeSet<_>>();
    assert_eq!(
        names.len(),
        unique.len(),
        "ui.toml contains duplicate {kind} names"
    );
    unique
}

fn validate(manifest: &Manifest) {
    let declared_core = unique_names("core fixture", manifest.core.iter().map(String::as_str));
    let actual_core = read_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/core"))
        .expect("ui/core should be readable")
        .filter_map(|entry| {
            let entry = entry.expect("ui/core entries should be readable");
            entry
                .file_type()
                .expect("ui/core entry types should be readable")
                .is_dir()
                .then(|| entry.file_name().to_string_lossy().into_owned())
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(declared_core, actual_core, "ui.toml core inventory drifted");

    let declared_examples = unique_names(
        "example",
        manifest.example.iter().map(|example| example.name.as_str()),
    );
    for example in &manifest.example {
        assert!(
            !example.lints.is_empty(),
            "example `{}` must enable at least one lint",
            example.name
        );
    }
    let cargo: CargoManifest =
        toml::from_str(include_str!("../Cargo.toml")).expect("Cargo.toml should be valid");
    let cargo_examples = unique_names(
        "Cargo example",
        cargo.example.iter().map(|example| example.name.as_str()),
    );
    assert_eq!(
        declared_examples, cargo_examples,
        "ui.toml example inventory drifted"
    );
}

fn enable_lints(test: &mut Test, lints: &[String]) {
    for lint in lints {
        let lint = format!("rlib::{lint}");
        test.rustc_flags(["-W", lint.as_str()]);
    }
}

fn run_example(manifest: &Manifest, example: &Example) {
    let mut test = Test::example(PACKAGE, &example.name);
    test.rustc_flags(&manifest.rustc_flags);
    enable_lints(&mut test, &example.lints);
    if let Some(rustc_flags) = &example.rustc_flags {
        test.rustc_flags(rustc_flags);
    }
    if let Some(dylint_toml) = &example.dylint_toml {
        test.dylint_toml(dylint_toml);
    }
    test.run();
}

fn run(manifest: &Manifest, selected: Option<&str>) {
    let core_selected = selected.filter(|selected| {
        manifest
            .core
            .iter()
            .any(|fixture| fixture.as_str() == *selected)
    });
    let example_selected = selected.and_then(|selected| {
        manifest
            .example
            .iter()
            .find(|example| example.name == selected)
    });
    assert!(
        selected.is_none() || core_selected.is_some() || example_selected.is_some(),
        "unknown UI fixture `{}`",
        selected.unwrap_or_default()
    );

    if example_selected.is_none() {
        let source = core_selected.map_or_else(
            || "ui/core".to_owned(),
            |fixture| format!("ui/core/{fixture}"),
        );
        let mut test = Test::src_base(PACKAGE, source);
        test.rustc_flags(&manifest.rustc_flags);
        test.rustc_flags(&manifest.core_rustc_flags);
        enable_lints(&mut test, &manifest.core_lints);
        test.run();
    }

    for example in example_selected.into_iter().chain(
        selected
            .is_none()
            .then_some(manifest.example.as_slice())
            .into_iter()
            .flatten(),
    ) {
        run_example(manifest, example);
    }
}

#[test]
fn fixture_ui() {
    // The proxy child owns the test result; the parent only propagates its status.
    if should_rerun_with_all_features() {
        return;
    }

    let manifest = manifest();
    validate(&manifest);
    run(&manifest, selected_fixture().as_deref());
}

// -----------------------------------------------------------------------------
// ShouldRerunWithAllFeatures: Static proxy process boundary
// -----------------------------------------------------------------------------

#[cfg(unix)]
fn should_rerun_with_all_features() -> bool {
    // The child already has the proxy environment and must execute fixtures directly.
    if var_os(CHILD_PROCESS).is_some() {
        return false;
    }

    // Re-enter through checked-in proxies so Dylint's preliminary build retains all features.
    let support = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/support");
    let path = join_paths(once(support).chain(split_paths(
        &var_os("PATH").expect("test process should have PATH"),
    )))
    .expect("Cargo proxy PATH should be valid");

    // Launch only this test and preserve the real tools behind the checked-in proxies.
    let executable = current_exe().expect("UI test executable should be available");
    let mut command = Command::new(executable);
    command.args(["fixture_ui", "--exact", "--nocapture"]);
    command.env(CHILD_PROCESS, "1");
    command.env(
        "RLIB_LINT_REAL_CARGO",
        var_os("CARGO").expect("Cargo should expose its executable path"),
    );
    command.env(
        "RLIB_LINT_REAL_RUSTC",
        var_os("RUSTC").expect("Cargo should expose the Rust compiler path"),
    );
    command.env("DYLINT_TOML", include_str!("../../../dylint.toml"));
    command.env("PATH", path);

    // Propagate the child result through the ordinary test assertion.
    let status = command.status().expect("all-feature UI child should start");
    assert!(status.success(), "all-feature UI child failed");
    true
}

#[cfg(not(unix))]
fn rerun_with_all_features() -> bool {
    panic!("Dylint UI tests currently require the Unix Cargo proxy")
}
