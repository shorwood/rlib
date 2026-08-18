//! Public lint documentation contract tests.

use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};

// -----------------------------------------------------------------------------
// DocumentedSetting: Expected public configuration entry
// -----------------------------------------------------------------------------

/// One documented configuration setting.
struct DocumentedSetting {
    /// Public configuration key.
    key: &'static str,
    /// Display form of the default value.
    default: &'static str,
}

// -----------------------------------------------------------------------------
// ConfigurableLint: Expected settings for one lint
// -----------------------------------------------------------------------------

/// Expected settings for one configurable lint.
struct ConfigurableLint {
    /// Family-qualified lint directory.
    name: &'static str,
    /// Complete public settings list.
    settings: &'static [DocumentedSetting],
}

// -----------------------------------------------------------------------------
// Family: Public lint family metadata
// -----------------------------------------------------------------------------

/// One public lint family and its Cargo feature.
struct Family {
    /// Source directory and lint-group suffix.
    name: &'static str,
    /// Human-readable family title.
    title: &'static str,
    /// Cargo feature that makes the family available.
    feature: &'static str,
}

// -----------------------------------------------------------------------------
// DocumentContext: Source and identity for one lint page
// -----------------------------------------------------------------------------

/// Source and identity used by every check on one lint page.
struct DocumentContext<'document> {
    /// Rust lint name.
    name: &'document str,
    /// Registration family.
    family: &'document str,
    /// Lint implementation source.
    source: &'document str,
    /// Public README contents.
    readme: &'document str,
}

impl<'document> DocumentContext<'document> {
    /// Returns the family-qualified lint directory.
    fn relative_name(&self) -> String {
        format!("{}/{}", self.family, self.name)
    }

    /// Verifies the title and fixed heading order.
    fn verify_page_structure(&self) {
        let DocumentContext {
            name,
            family,
            source,
            readme,
        } = self;
        assert!(
            source.contains("#[doc = include_str!(\"README.md\")]"),
            "{family}/{name} must include its README as lint documentation"
        );
        assert_eq!(
            readme.lines().next(),
            Some(format!("# `rlib::{name}`").as_str()),
            "{family}/{name} must lead with its public lint name"
        );

        let headings = readme
            .lines()
            .filter(|line| line.starts_with("## "))
            .collect::<Vec<_>>();
        assert_eq!(
            headings, LINT_HEADINGS,
            "{family}/{name} uses a noncanonical page structure"
        );
        let example_headings = readme
            .lines()
            .filter(|line| line.starts_with("### "))
            .collect::<Vec<_>>();
        assert_eq!(
            example_headings, EXAMPLE_HEADINGS,
            "{family}/{name} uses noncanonical example headings"
        );
    }

    /// Verifies the one-sentence summary and returns it for the family index.
    fn verify_summary(&self) -> &'document str {
        let name = self.name;
        let family = self.family;
        let readme = self.readme;
        let summary = documentation_section(readme, LINT_HEADINGS[0]..LINT_HEADINGS[1]);
        assert!(!summary.is_empty(), "{family}/{name} needs a summary");
        let summary_ending = summary.chars().last();
        assert!(
            !summary.contains("\n\n")
                && summary_ending.is_some_and(|ending| ['.', '!', '?'].contains(&ending)),
            "{family}/{name} summary must be one sentence"
        );
        assert_eq!(
            summary.matches('`').count() % 2,
            0,
            "{family}/{name} summary has unmatched inline code"
        );
        summary
    }

    /// Verifies public metadata and returns the values reused by the family index.
    fn verify_metadata(&self) -> VerifiedMetadata<'document> {
        let name = self.name;
        let family = self.family;
        let source = self.source;
        let readme = self.readme;
        let metadata = documentation_section(readme, LINT_HEADINGS[1]..LINT_HEADINGS[2]);
        let value = |field: &str| {
            let prefix = format!("| {field} | ");
            metadata
                .lines()
                .find_map(|line| line.strip_prefix(&prefix))
                .and_then(|value| value.strip_suffix(" |"))
                .unwrap_or_else(|| panic!("metadata must define {field}"))
        };
        assert!(
            metadata.starts_with("| Field | Value |\n| --- | --- |"),
            "{family}/{name} needs the fixed metadata table"
        );
        assert_eq!(
            metadata.lines().count(),
            7,
            "{family}/{name} metadata must contain exactly the five public fields"
        );
        assert_eq!(value("Group"), format!("`rlib::{family}`"));
        let expected_feature = if family == "core" {
            "always".to_owned()
        } else {
            format!("`{family}`")
        };
        assert_eq!(value("Cargo feature"), expected_feature);
        assert_eq!(value("Default level"), "`warn`");
        assert!(
            source.contains("Warn,"),
            "{family}/{name} must default to warn"
        );

        let purpose = value("Purpose");
        assert!(
            LINT_PURPOSES.contains(&purpose),
            "{family}/{name} has unknown purpose {purpose}"
        );
        let fix = value("Fix");
        assert!(
            LINT_FIXES.contains(&fix),
            "{family}/{name} has unknown fix value {fix}"
        );
        match fix {
            "Automatic" => assert!(
                source.contains("Applicability::MachineApplicable")
                    && !source.contains("Applicability::MaybeIncorrect"),
                "{family}/{name} must provide machine-applicable suggestions"
            ),
            "Needs review" => assert!(
                source.contains("Applicability::MaybeIncorrect"),
                "{family}/{name} must provide a reviewable suggestion"
            ),
            "Manual" => assert!(
                !source.contains("Applicability::"),
                "{family}/{name} offers a suggestion and cannot be marked manual"
            ),
            _ => unreachable!(),
        }
        VerifiedMetadata { purpose, fix }
    }

    /// Verifies examples and all required explanatory sections.
    fn verify_explanations(&self) {
        let name = self.name;
        let family = self.family;
        let readme = self.readme;
        let examples = documentation_section(readme, LINT_HEADINGS[4]..LINT_HEADINGS[5]);
        for bounds in [
            EXAMPLE_HEADINGS[0]..EXAMPLE_HEADINGS[1],
            EXAMPLE_HEADINGS[1].."",
        ] {
            assert!(
                documentation_section(examples, bounds).contains("```rust"),
                "{family}/{name} needs a Rust example under each example heading"
            );
        }

        for (heading_index, following_heading) in [
            (2, LINT_HEADINGS[3]),
            (3, LINT_HEADINGS[4]),
            (5, LINT_HEADINGS[6]),
            (6, LINT_HEADINGS[7]),
            (8, LINT_HEADINGS[9]),
            (9, ""),
        ] {
            assert!(
                !documentation_section(readme, LINT_HEADINGS[heading_index]..following_heading)
                    .is_empty(),
                "{family}/{name} has an empty {} section",
                LINT_HEADINGS[heading_index]
            );
        }
    }

    /// Verifies every public setting key and default.
    fn verify_settings(&self) {
        let relative_name = self.relative_name();
        let readme = self.readme;
        let settings = documentation_section(readme, LINT_HEADINGS[7]..LINT_HEADINGS[8]);
        let expected_settings = CONFIGURABLE_LINTS
            .iter()
            .find(|lint| lint.name == relative_name)
            .map(|lint| lint.settings);
        if let Some(expected_settings) = expected_settings {
            assert!(
                settings
                    .starts_with("| Key | Type | Default | Effect |\n| --- | --- | --- | --- |"),
                "{relative_name} must document settings in the fixed table"
            );
            let rows = settings
                .lines()
                .filter(|line| line.starts_with("| `"))
                .collect::<Vec<_>>();
            assert_eq!(
                rows.len(),
                expected_settings.len(),
                "{relative_name} must document every setting exactly once"
            );
            for setting in expected_settings {
                let prefix = format!("| `{}` | ", setting.key);
                let row = rows
                    .iter()
                    .find(|row| row.starts_with(&prefix))
                    .unwrap_or_else(|| {
                        panic!("{relative_name} must document setting {}", setting.key)
                    });
                let cells = row.split('|').map(str::trim).collect::<Vec<_>>();
                assert_eq!(
                    cells.get(3),
                    Some(&setting.default),
                    "{relative_name} documents the wrong default for {}",
                    setting.key
                );
            }
        } else {
            assert_eq!(
                settings, "This lint has no behavior-specific settings.",
                "{relative_name} is not configurable"
            );
        }
    }

    /// Verifies plain language and every related-lint link.
    fn verify_prose_and_links(&self, directory: &Path) {
        let relative_name = self.relative_name();
        let readme = self.readme;
        let prose = documentation_prose_only(readme);
        for term in INTERNAL_TERMS {
            assert!(
                !prose.contains(term),
                "{relative_name} exposes compiler implementation term {term}"
            );
        }
        let lowercase_prose = prose.to_lowercase();
        for phrase in VAGUE_PHRASES {
            assert!(
                !lowercase_prose.contains(phrase),
                "{relative_name} uses vague phrase {phrase}"
            );
        }

        let related = documentation_section(readme, LINT_HEADINGS[9].."");
        for suffix in related.split("](").skip(1) {
            let link = suffix
                .split_once(')')
                .map(|(link, _)| link)
                .expect("related lint link should close");
            let target = directory.join(link);
            assert_ne!(
                target
                    .canonicalize()
                    .expect("related lint path should resolve"),
                directory
                    .join("README.md")
                    .canonicalize()
                    .expect("current README path should resolve"),
                "{relative_name} must not link to itself"
            );
            assert!(target.is_file(), "{relative_name} has broken link {link}");
        }
    }
}

// -----------------------------------------------------------------------------
// VerifiedMetadata: Reusable values from a checked metadata table
// -----------------------------------------------------------------------------

/// Metadata values reused after validation.
struct VerifiedMetadata<'document> {
    /// User-facing reason for the lint.
    purpose: &'document str,
    /// Compiler assistance available for fixes.
    fix: &'document str,
}

// -----------------------------------------------------------------------------
// LintDocument: Verified facts used by the public catalogs
// -----------------------------------------------------------------------------

/// Parsed facts reused by catalog checks.
struct LintDocument {
    /// Directory containing the lint.
    directory: PathBuf,
    /// Rust lint name.
    name: String,
    /// Registration family.
    family: String,
    /// One-sentence page summary.
    summary: String,
    /// User-facing reason for the lint.
    purpose: String,
    /// Compiler assistance available for fixes.
    fix: String,
}

impl LintDocument {
    /// Reads and verifies one lint page.
    fn from_directory(directory: &Path) -> Self {
        // Resolve the lint and family names from their source directory.
        let name = directory
            .file_name()
            .and_then(|name| name.to_str())
            .expect("lint name should be UTF-8");
        let family = directory
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .expect("family name should be UTF-8");

        // Read the implementation and public page before running the contract checks.
        let source = fs::read_to_string(directory.join("mod.rs"))
            .expect("lint implementation should be readable");
        let readme = fs::read_to_string(directory.join("README.md"))
            .expect("active lint must have README.md");
        let document = DocumentContext {
            name,
            family,
            source: &source,
            readme: &readme,
        };

        document.verify_page_structure();
        let summary = document.verify_summary();
        let metadata = document.verify_metadata();
        document.verify_explanations();
        document.verify_settings();
        document.verify_prose_and_links(directory);

        Self {
            directory: directory.to_owned(),
            name: name.to_owned(),
            family: family.to_owned(),
            summary: summary.to_owned(),
            purpose: metadata.purpose.to_owned(),
            fix: metadata.fix.to_owned(),
        }
    }
}

// -----------------------------------------------------------------------------
// Lint: Required public page metadata
// -----------------------------------------------------------------------------

/// Required second-level headings in every lint README.
const LINT_HEADINGS: [&str; 10] = [
    "## Summary",
    "## At a glance",
    "## What it catches",
    "## Why this matters",
    "## Examples",
    "## What it skips",
    "## When to turn it off",
    "## Settings",
    "## Known limitations",
    "## Related lints",
];

/// Supported user-facing reasons for a lint.
const LINT_PURPOSES: [&str; 5] = [
    "Correctness",
    "Safety",
    "API design",
    "Code clarity",
    "Style",
];

/// Supported descriptions of compiler-assisted fixes.
const LINT_FIXES: [&str; 3] = ["Automatic", "Needs review", "Manual"];

// -----------------------------------------------------------------------------
// ExampleHeadings: Required before-and-after examples
// -----------------------------------------------------------------------------

/// Required example headings in every lint README.
const EXAMPLE_HEADINGS: [&str; 2] = ["### Triggers the lint", "### Use this instead"];

// -----------------------------------------------------------------------------
// InternalTerms: Compiler vocabulary excluded from public docs
// -----------------------------------------------------------------------------

/// Compiler implementation names that do not belong in user documentation.
const INTERNAL_TERMS: [&str; 5] = ["EarlyLintPass", "LateLintPass", "DefId", "HirId", "TyCtxt"];

// -----------------------------------------------------------------------------
// VaguePhrases: Indirect wording excluded from public docs
// -----------------------------------------------------------------------------

/// Vague implementation language that should be replaced with a direct explanation.
const VAGUE_PHRASES: [&str; 8] = [
    "policy surface",
    "internal coordination",
    "authored contract",
    "protocol boilerplate",
    "nominal value",
    "comparison provenance",
    "compilation-local",
    "abstract boundary",
];

// -----------------------------------------------------------------------------
// Configurable: Exact public setting keys and defaults
// -----------------------------------------------------------------------------

/// Builds named configuration records without repeating field labels in the table below.
macro_rules! configurable_lint {
    ($name:literal, &[$(($key:literal, $default:literal)),+ $(,)?] $(,)?) => {
        ConfigurableLint {
            name: $name,
            settings: &[
                $(DocumentedSetting { key: $key, default: $default }),+
            ],
        }
    };
}

/// Lints whose behavior is controlled by public settings, with exact defaults.
const CONFIGURABLE_LINTS: &[ConfigurableLint] = &[
    configurable_lint!(
        "bon/bon_required_builder_members_breaking_compatibility",
        &[("bon-api-baseline", "`[]`")],
    ),
    configurable_lint!(
        "core/deeply_nested_control_flow",
        &[("control-flow-depth-threshold", "`2`")],
    ),
    configurable_lint!(
        "core/incoherent_extension_traits",
        &[("extension-trait-methods-threshold", "`8`")],
    ),
    configurable_lint!(
        "core/long_method_chains",
        &[("method-chain-calls-threshold", "`3`")],
    ),
    configurable_lint!(
        "core/missing_code_phase_comments",
        &[("function-phase-lines-threshold", "`7`")],
    ),
    configurable_lint!(
        "core/needlessly_nested_control_flow",
        &[("control-flow-depth-threshold", "`2`")],
    ),
    configurable_lint!(
        "core/overloaded_declaration_sections",
        &[("declarations-per-section-threshold", "`5`")],
    ),
    configurable_lint!(
        "core/oversized_match_arms",
        &[("match-arm-lines-threshold", "`7`")],
    ),
    configurable_lint!(
        "derive_more/derive_more_manual_error_impls",
        &[("error-implementation-provider", "not set")],
    ),
    configurable_lint!(
        "derive_more/derive_more_manual_variant_accessors",
        &[("enum-variant-predicate-provider", "not set")],
    ),
    configurable_lint!(
        "framework/framework_resolution_required",
        &[
            ("error-implementation-provider", "not set"),
            ("error-variant-conversion-provider", "not set"),
            ("enum-variant-collection-provider", "not set"),
            ("enum-variant-predicate-provider", "not set"),
            ("enum-string-parsing-provider", "not set"),
            ("enum-display-provider", "not set"),
        ],
    ),
    configurable_lint!(
        "leptos/leptos_excessive_component_composition_depth",
        &[("leptos-component-composition-depth-threshold", "`10`")],
    ),
    configurable_lint!(
        "leptos/leptos_excessive_component_props",
        &[("leptos-component-props-threshold", "`6`")],
    ),
    configurable_lint!(
        "leptos/leptos_excessively_nested_views",
        &[
            ("leptos-view-nesting-depth-threshold", "`7`"),
            ("leptos-view-control-flow-depth-threshold", "`3`"),
        ],
    ),
    configurable_lint!(
        "leptos/leptos_fragmented_reactive_state",
        &[("leptos-reactive-primitives-threshold", "`4`")],
    ),
    configurable_lint!(
        "leptos/leptos_missing_view_attribute_group_comments",
        &[("leptos-unnamed-view-attribute-complexity-threshold", "`6`")],
    ),
    configurable_lint!(
        "leptos/leptos_missing_view_section_comments",
        &[("leptos-unnamed-view-complexity-threshold", "`4`")],
    ),
    configurable_lint!(
        "leptos/leptos_noncanonical_view_formatting",
        &[("leptos-view-max-width", "`100`")],
    ),
    configurable_lint!(
        "leptos/leptos_overpopulated_component_modules",
        &[("leptos-components-per-module-threshold", "`8`")],
    ),
    configurable_lint!(
        "leptos/leptos_oversized_event_handlers",
        &[
            ("leptos-event-handler-statements-threshold", "`3`"),
            ("leptos-event-handler-control-flow-depth-threshold", "`1`"),
        ],
    ),
    configurable_lint!(
        "leptos/leptos_oversized_reactive_setups",
        &[("leptos-setup-statements-threshold", "`8`")],
    ),
    configurable_lint!(
        "leptos/leptos_oversized_view_attribute_groups",
        &[("leptos-view-attribute-group-complexity-threshold", "`4`")],
    ),
    configurable_lint!(
        "leptos/leptos_oversized_view_sections",
        &[("leptos-view-section-complexity-threshold", "`4`")],
    ),
    configurable_lint!(
        "leptos/leptos_repeated_view_fragments",
        &[
            ("leptos-repeated-view-fragment-nodes-threshold", "`6`"),
            ("leptos-repeated-view-fragment-occurrences-threshold", "`2`"),
        ],
    ),
    configurable_lint!(
        "leptos/leptos_server_functions_without_authorization_boundaries",
        &[
            ("leptos-sensitive-call-terms", "built-in list"),
            ("leptos-authorization-functions", "`[]`"),
            ("leptos-protected-endpoint-attributes", "`[]`"),
            ("leptos-public-endpoint-attributes", "`[]`"),
        ],
    ),
    configurable_lint!(
        "leptos_styling/leptos_styling_noncanonical_css",
        &[("leptos-css-max-width", "`100`")],
    ),
    configurable_lint!(
        "miette/miette_generic_diagnostic_help",
        &[("miette-generic-help-phrases", "built-in list")],
    ),
    configurable_lint!(
        "strum/strum_manual_enum_iteration",
        &[("enum-variant-collection-provider", "not set")],
    ),
    configurable_lint!(
        "strum/strum_manual_enum_predicates",
        &[("enum-variant-predicate-provider", "not set")],
    ),
    configurable_lint!(
        "strum/strum_manual_enum_string_conversions",
        &[("enum-display-provider", "not set")],
    ),
    configurable_lint!(
        "strum/strum_manual_enum_string_parsers",
        &[("enum-string-parsing-provider", "not set")],
    ),
    configurable_lint!(
        "strum/strum_manual_variant_arrays",
        &[("enum-variant-collection-provider", "not set")],
    ),
    configurable_lint!(
        "thiserror/thiserror_manual_error_impls",
        &[("error-implementation-provider", "not set")],
    ),
    configurable_lint!(
        "thiserror/thiserror_manual_from_error_variants",
        &[("error-variant-conversion-provider", "not set")],
    ),
];

// -----------------------------------------------------------------------------
// Families: Public lint families and Cargo features
// -----------------------------------------------------------------------------

/// Every public lint family.
const FAMILIES: &[Family] = &[
    Family {
        name: "core",
        title: "Core",
        feature: "always",
    },
    Family {
        name: "bon",
        title: "Bon",
        feature: "bon",
    },
    Family {
        name: "derive_more",
        title: "Derive More",
        feature: "derive_more",
    },
    Family {
        name: "framework",
        title: "Framework",
        feature: "framework",
    },
    Family {
        name: "leptos",
        title: "Leptos",
        feature: "leptos",
    },
    Family {
        name: "leptos_styling",
        title: "Leptos Styling",
        feature: "leptos_styling",
    },
    Family {
        name: "miette",
        title: "Miette",
        feature: "miette",
    },
    Family {
        name: "serde",
        title: "Serde",
        feature: "serde",
    },
    Family {
        name: "strum",
        title: "Strum",
        feature: "strum",
    },
    Family {
        name: "thiserror",
        title: "thiserror",
        feature: "thiserror",
    },
];

// -----------------------------------------------------------------------------
// Documentation: Page discovery and text extraction
// -----------------------------------------------------------------------------

/// Returns the root of the lint source tree.
fn documentation_rules_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/rules")
}

/// Extracts content between two required headings.
fn documentation_section<'document>(
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
    remainder[..end].trim()
}

/// Removes fenced examples before checking prose vocabulary.
fn documentation_prose_only(document: &str) -> String {
    let mut fenced = false;
    document
        .lines()
        .filter(|line| {
            if line.starts_with("```") {
                fenced = !fenced;
                false
            } else {
                !fenced
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Collects every directory that declares a Dylint lint.
fn documentation_lint_directories() -> Vec<PathBuf> {
    let root = documentation_rules_root();
    let mut directories = Vec::new();
    for family in FAMILIES {
        for entry in fs::read_dir(root.join(family.name)).expect("family should be readable") {
            let path = entry.expect("family entry should be readable").path();
            let module = path.join("mod.rs");
            let Ok(source) = fs::read_to_string(module) else {
                continue;
            };
            if !source.contains("crate::impl_") {
                continue;
            }
            directories.push(path);
        }
    }
    directories.sort();
    directories
}

// -----------------------------------------------------------------------------
// Catalog: Public page and family coverage tests
// -----------------------------------------------------------------------------

/// Requires every active lint to use the complete public documentation contract.
#[test]
fn catalog_lint_documentation_is_complete_and_plain() {
    for directory in documentation_lint_directories() {
        LintDocument::from_directory(&directory);
    }
}

/// Keeps family landing pages complete and synchronized with lint pages.
#[test]
fn catalog_family_indexes_cover_every_active_lint() {
    let documents = documentation_lint_directories()
        .iter()
        .map(|directory| LintDocument::from_directory(directory))
        .collect::<Vec<_>>();
    let root = documentation_rules_root();

    let catalog = fs::read_to_string(root.join("README.md"))
        .expect("rules/README.md must provide the root lint catalog");
    for family in FAMILIES {
        let link = format!("[{}](./{}/README.md)", family.title, family.name);
        assert!(catalog.contains(&link), "root catalog must link {link}");

        let index = fs::read_to_string(root.join(family.name).join("README.md"))
            .unwrap_or_else(|_| panic!("{} must have a family README", family.name));
        assert_eq!(
            index.lines().next(),
            Some(format!("# {} lints", family.title).as_str())
        );
        let headings = index
            .lines()
            .filter(|line| line.starts_with("## "))
            .collect::<Vec<_>>();
        assert_eq!(headings, ["## Summary", "## How to enable", "## Lints"]);
        if family.feature == "always" {
            assert!(index.contains("always available"));
        } else {
            assert!(index.contains(format!("`{}` Cargo feature", family.feature).as_str()));
        }

        let family_documents = documents
            .iter()
            .filter(|document| document.family == family.name)
            .collect::<Vec<_>>();
        let rows = index
            .lines()
            .filter(|line| line.starts_with("| [`rlib::"))
            .count();
        assert_eq!(
            rows,
            family_documents.len(),
            "{} index must list every active lint exactly once",
            family.name
        );
        for document in family_documents {
            let relative = document
                .directory
                .file_name()
                .and_then(|name| name.to_str())
                .expect("lint directory name should be UTF-8");
            let escaped_summary = document.summary.replace('|', "\\|");
            let row = format!(
                "| [`rlib::{}`](./{relative}/README.md) | {} | {} | {} |",
                document.name, escaped_summary, document.purpose, document.fix
            );
            assert!(
                index.contains(&row),
                "{} index is missing or has stale row: {row}",
                family.name
            );
        }
    }
}
