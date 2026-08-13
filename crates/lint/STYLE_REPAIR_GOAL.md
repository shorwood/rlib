# Restore the lint crate's intended code style

## Objective

Repair all code-style damage introduced while making the repository self-linting, especially in commit `efa5d77`. Audit the complete lint crate rather than stopping when validation first becomes green. The finished source should read as deliberately authored.

## Canonical reference

Use `src/rules/core/ad_hoc_collection_construction/mod.rs` as the primary structural reference. It demonstrates the intended separation between diagnostics, analysis, and lint-pass policy; meaningful type and field documentation; consistent use of `Violation`; and responsibility-based sections.

Use section dividers in this form:

```rust
// -----------------------------------------------------------------------------
// Abstraction: Concrete responsibility
// -----------------------------------------------------------------------------
```

The title must identify both the abstraction and its responsibility. Imports do not need a section.

## Structural policy

Add sections when a module contains multiple conceptual responsibilities, even when it has few declarations. Common rule-module sections include:

- `Violation: <diagnostic meaning>`
- `<Candidate or Analysis>: <evidence collected>`
- `<Helper abstraction>: <specific responsibility>`
- `<LintPassName>: <enforced policy>`
- `Configuration: <configured policy>`

Do not add sections around every declaration, between tightly coupled helpers, merely because a declaration category changes, or solely because a numeric threshold was crossed. Avoid generic titles such as `Types`, `Helpers`, `Implementation`, and `Utilities`. Small modules with one responsibility may remain unsectioned.

Section policy must account for conceptual responsibility. Declaration count may inform a configurable scale limit, but it must not be the sole definition of good organization.

## Documentation and comments

Remove mechanical documentation such as:

- `Carries the X state used by this analysis`
- `Stores the value used by this analysis`
- `Performs the operation for this value`

Documentation should explain the domain concept, retained evidence, enforced policy, semantic distinction, or reason a field exists. It must not merely restate an identifier or type.

Remove mechanical phase narration such as:

- `Prepare the values used by this stage`
- `Reject inputs that do not satisfy this stage`
- `Perform the next step of the analysis`
- `Classify the current analyze_candidate`
- `Register the next cohesive set of lint policies`

Comments should explain non-obvious intent, precedence, safety, filtering, invariants, or genuine multi-statement workflow boundaries. Do not introduce comments merely to satisfy a lint. Extract a meaningful operation when a function truly owns several responsibilities.

## Diagnostic architecture

Use focused `Violation` values and the appropriate `EarlyViolation` or `LateViolation` trait. Keep evidence discovery separate from diagnostic formatting. Primary messages must identify the violated contract, rationales must explain the user-visible consequence, and remediation must be concrete and library-aware.

Never expose internal analysis names such as `analyze_candidate`, `analyze_operator`, `is_selected`, or `is_forwarding` in diagnostics or public documentation.

## Style-lint audit

Review the README, implementation, diagnostics, configuration, and UI coverage of at least these rules:

- `missing_section_dividers`
- `overloaded_declaration_sections`
- `malformed_section_dividers`
- `mismatched_section_divider_prefixes`
- `duplicate_section_divider_prefixes`
- `misordered_module_declarations`
- `misordered_type_declarations`
- `misordered_inherent_impl_items`
- `missing_code_phase_comments`
- `malformed_code_phase_comments`
- `long_method_chains`
- `undocumented_items`
- `unnecessarily_broad_visibility`

For each lint, state its human-readable policy, compare it with the canonical reference, and add positive and negative cases covering both over-application and under-application. Fix the lint or diagnostic when its recommendation would degrade source code. Do not bless an incorrect snapshot.

## Repository-wide audit

Inspect every Rust source file under `crates/lint/src`, including all framework layers, registration, shared utilities, and test helpers. Review the full `efa5d77^..efa5d77` diff to find damage not discoverable through text search.

Search for the mechanical phrases and leaked identifiers listed above. Review every match in context rather than applying blind replacements.

For each questionable location:

1. Read the surrounding abstraction and relevant diagnostic.
2. Identify the actual responsibility represented by the code.
3. Decide whether the source or lint is wrong.
4. Repair the root cause.
5. Organize distinct responsibilities into meaningful sections.
6. Rewrite documentation and comments in domain language.
7. Ensure the result is not another lint-appeasement artifact.
8. Review the final module for human readability.

Resolve conflicts between enabled lints in their implementations or through existing explicit configuration knobs. Do not encode compromises as awkward source code.

## Constraints

Do not:

- add project-lint `allow` or `expect` attributes;
- disable lints or exclude source from repository self-validation;
- weaken a lint merely to hide a repository violation;
- mechanically add or remove all section dividers;
- create trivial helpers solely to satisfy line-count limits;
- broaden production visibility only for tests;
- change framework conflict resolution away from explicit configuration;
- remove library prefixes from framework-aware lint names;
- modify or stage unrelated user work, including `nix/dylint-driver.nix` and `AGENTS.md`.

## Validation

The completed work must pass:

```sh
cargo fmt --all -- --check
cargo check --workspace --lib --tests --all-features
cargo check --workspace --lib --no-default-features
cargo test --workspace --lib --tests --all-features
cargo-clippy clippy --workspace --lib --tests --all-features -- -D warnings
cargo build -p rlib-lint --all-features
DYLINT_LIBRARY_PATH="$PWD/target/debug" DYLINT_RUSTFLAGS="-Dwarnings" \
  cargo dylint --lib rlib_lint --workspace -- --all-features
just ok
```

Also confirm that every active lint has accurate public documentation, UI snapshots use public terminology, no project lint is suppressed, all lints remain enabled during self-validation, and every affected source file was reviewed.

## Completion evidence

Report:

- recurring damage patterns found;
- style-lint implementation changes;
- representative repairs in every lint layer;
- examples of removed over-sectioning and restored missing sections;
- corrected diagnostic wording;
- false-positive and false-negative tests added;
- exact validation results;
- confirmation that no project-lint suppressions were added;
- confirmation that the repository-wide audit was completed;
- the final commit hash.

Commit only after the complete audit passes. Exclude unrelated existing changes.
