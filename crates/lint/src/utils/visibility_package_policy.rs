use std::env::var;
use std::fs::read_to_string;
use std::path::Path;

use serde::Deserialize;

// -----------------------------------------------------------------------------
// Cargo: Publication metadata
// -----------------------------------------------------------------------------
/// Package publication setting interpreted conservatively.
#[derive(Deserialize)]
struct CargoPackage {
    /// Boolean or registry-list form accepted by Cargo.
    publish: Option<toml::Value>,
}

/// Cargo fields needed to distinguish publishable and closed packages.
#[derive(Deserialize)]
struct CargoManifest {
    /// Package table omitted only by virtual workspace manifests.
    package: Option<CargoPackage>,
    /// Explicit library target metadata.
    lib: Option<toml::Table>,
}

/// Presence of Cargo's implicit `src/lib.rs` library target.
#[derive(Clone, Copy)]
enum CargoImplicitLibrary {
    /// No implicit library target exists.
    Absent,
    /// The conventional implicit library target exists.
    Present,
}

impl CargoImplicitLibrary {
    /// Classifies whether a manifest directory contains Cargo's conventional library source.
    fn at_manifest(path: &Path) -> Self {
        if path
            .parent()
            .is_some_and(|root| root.join("src/lib.rs").is_file())
        {
            Self::Present
        } else {
            Self::Absent
        }
    }
}

// -----------------------------------------------------------------------------
// VisibilityPackagePolicy: External api intent
// -----------------------------------------------------------------------------
/// Publication policy inferred from the target package manifest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum VisibilityPackagePolicy {
    /// Package explicitly refuses publication and is treated as locally closed.
    Closed,
    /// Package may have downstream consumers whose uses are unobservable.
    Publishable,
}

impl VisibilityPackagePolicy {
    /// Loads the target package policy, preserving public APIs when metadata is unavailable.
    pub(super) fn for_current_package() -> Self {
        // Resolve the manifest supplied to the target compiler invocation.
        let Ok(manifest_directory) = var("CARGO_MANIFEST_DIR") else {
            return Self::Publishable;
        };
        Self::from_manifest_path(&Path::new(&manifest_directory).join("Cargo.toml"))
    }

    /// Interprets one package manifest without treating malformed metadata as permission to narrow.
    fn from_manifest_path(path: &Path) -> Self {
        // An unreadable manifest cannot prove that the package is closed to dependents.
        let Ok(source) = read_to_string(path) else {
            return Self::Publishable;
        };
        let implicit_library = CargoImplicitLibrary::at_manifest(path);
        Self::from_manifest_source(&source, implicit_library)
    }

    /// Treats only the explicit boolean `false` form as a closed package contract.
    fn from_manifest_source(source: &str, implicit_library: CargoImplicitLibrary) -> Self {
        // Parse only publication metadata while accepting unrelated cargo fields.
        let Ok(manifest) = toml::from_str::<CargoManifest>(source) else {
            return Self::Publishable;
        };
        let has_explicit_linkable_library = manifest.lib.as_ref().is_some_and(|library| {
            library.get("crate-type").is_none_or(|crate_types| {
                crate_types.as_array().is_some_and(|crate_types| {
                    crate_types.iter().any(|crate_type| {
                        crate_type
                            .as_str()
                            .is_some_and(|crate_type| matches!(crate_type, "lib" | "rlib"))
                    })
                })
            })
        });
        let has_library = has_explicit_linkable_library
            || matches!(implicit_library, CargoImplicitLibrary::Present);
        let publish = manifest.package.and_then(|package| package.publish);
        if publish
            .as_ref()
            .is_some_and(|value| value.as_bool() == Some(false))
            && !has_library
        {
            Self::Closed
        } else {
            Self::Publishable
        }
    }

    /// Returns whether unobserved downstream Rust consumers must be preserved.
    pub(super) const fn is_preserving_exported_public_items(self) -> bool {
        matches!(self, Self::Publishable)
    }
}

// -----------------------------------------------------------------------------
// Tests: Unit tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::{CargoImplicitLibrary, VisibilityPackagePolicy};

    #[test]
    fn recognizes_explicitly_unpublished_packages() {
        let source = "[package]\nname = 'worker'\npublish = false\n";
        assert_eq!(
            VisibilityPackagePolicy::from_manifest_source(source, CargoImplicitLibrary::Absent),
            VisibilityPackagePolicy::Closed
        );
    }

    #[test]
    fn preserves_absent_registry_and_malformed_publication_metadata() {
        for source in [
            "[package]\nname = 'library'\n",
            "[package]\nname = 'library'\npublish = ['private']\n",
            "not valid toml",
        ] {
            assert_eq!(
                VisibilityPackagePolicy::from_manifest_source(source, CargoImplicitLibrary::Absent,),
                VisibilityPackagePolicy::Publishable
            );
        }
    }

    #[test]
    fn preserves_public_apis_for_unpublished_library_packages() {
        let explicit = "[package]\nname = 'protocol'\npublish = false\n[lib]\n";
        assert_eq!(
            VisibilityPackagePolicy::from_manifest_source(explicit, CargoImplicitLibrary::Absent,),
            VisibilityPackagePolicy::Publishable
        );
        let implicit = "[package]\nname = 'protocol'\npublish = false\n";
        assert_eq!(
            VisibilityPackagePolicy::from_manifest_source(implicit, CargoImplicitLibrary::Present,),
            VisibilityPackagePolicy::Publishable
        );
    }

    #[test]
    fn keeps_cdylib_only_packages_closed() {
        let source =
            "[package]\nname = 'plugin'\npublish = false\n[lib]\ncrate-type = ['cdylib']\n";
        assert_eq!(
            VisibilityPackagePolicy::from_manifest_source(source, CargoImplicitLibrary::Absent),
            VisibilityPackagePolicy::Closed
        );
    }
}
