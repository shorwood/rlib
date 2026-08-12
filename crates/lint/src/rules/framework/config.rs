use serde::Deserialize;

#[cfg(feature = "strum")]
use crate::rules::strum::utils::enumeration::CollectionProvider;

// -----------------------------------------------------------------------------
// DeriveResolutionConfig: Explicit framework provider policy
// -----------------------------------------------------------------------------

/// Workspace-wide provider choices for overlapping framework remediations.
#[derive(Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
#[allow(clippy::redundant_pub_crate)]
pub(crate) struct DeriveResolutionConfig {
    /// Provider selected for exhaustive enum variant collections.
    #[cfg(feature = "strum")]
    enum_variant_collection: Option<CollectionProvider>,
}

impl DeriveResolutionConfig {
    /// Returns the configured provider for exhaustive enum collections.
    #[cfg(feature = "strum")]
    pub(crate) const fn enum_variant_collection(&self) -> Option<CollectionProvider> {
        self.enum_variant_collection
    }
}

#[cfg(test)]
mod tests {
    use super::DeriveResolutionConfig;
    #[cfg(feature = "strum")]
    use crate::rules::strum::utils::enumeration::CollectionProvider;

    #[cfg(feature = "strum")]
    #[test]
    fn parses_explicit_strum_collection_provider() {
        let config = toml::from_str::<DeriveResolutionConfig>(
            r#"enum_variant_collection = "strum_variant_array""#,
        )
        .expect("known provider should parse");
        assert_eq!(
            config.enum_variant_collection,
            Some(CollectionProvider::StrumVariantArray)
        );
    }

    #[cfg(feature = "strum")]
    #[test]
    fn rejects_unknown_collection_provider() {
        assert!(
            toml::from_str::<DeriveResolutionConfig>(
                r#"enum_variant_collection = "dependency_order""#
            )
            .is_err()
        );
    }
}
