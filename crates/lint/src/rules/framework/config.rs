use serde::Deserialize;

#[cfg(feature = "thiserror")]
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorVariantConversionProvider {
    DeriveMoreFrom,
    ThiserrorFrom,
}

#[cfg(feature = "thiserror")]
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorImplementationProvider {
    DeriveMoreError,
    ThiserrorError,
}

#[cfg(feature = "strum")]
use crate::rules::strum::utils::authored_contracts::{DisplayProvider, StringParserProvider};
#[cfg(feature = "strum")]
use crate::rules::strum::utils::enumeration::CollectionProvider;
#[cfg(any(feature = "strum", feature = "derive_more"))]
use crate::utils::variant_methods::PredicateProvider;

// -----------------------------------------------------------------------------
// DeriveResolutionConfig: Explicit framework provider policy
// -----------------------------------------------------------------------------

/// Workspace-wide provider choices for overlapping framework remediations.
#[derive(Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct DeriveResolutionConfig {
    /// Provider selected for complete error implementations.
    #[cfg(feature = "thiserror")]
    error_implementation: Option<ErrorImplementationProvider>,
    /// Provider selected for source-bearing error-variant conversions.
    #[cfg(feature = "thiserror")]
    error_variant_conversion: Option<ErrorVariantConversionProvider>,
    /// Provider selected for exhaustive enum variant collections.
    #[cfg(feature = "strum")]
    enum_variant_collection: Option<CollectionProvider>,
    /// Provider selected for generated enum variant predicates.
    #[cfg(any(feature = "strum", feature = "derive_more"))]
    enum_variant_predicates: Option<PredicateProvider>,
    /// Provider selected for flat unit-enum string parsers.
    #[cfg(feature = "strum")]
    enum_string_parsing: Option<StringParserProvider>,
    /// Provider selected for enum `Display` implementations.
    #[cfg(feature = "strum")]
    enum_display: Option<DisplayProvider>,
}

impl DeriveResolutionConfig {
    #[cfg(feature = "thiserror")]
    pub(crate) const fn error_implementation(&self) -> Option<ErrorImplementationProvider> {
        self.error_implementation
    }
    #[cfg(feature = "thiserror")]
    pub(crate) const fn error_variant_conversion(&self) -> Option<ErrorVariantConversionProvider> {
        self.error_variant_conversion
    }
    /// Returns the configured provider for exhaustive enum collections.
    #[cfg(feature = "strum")]
    pub(crate) const fn enum_variant_collection(&self) -> Option<CollectionProvider> {
        self.enum_variant_collection
    }

    /// Returns the configured provider for enum variant predicates.
    #[cfg(any(feature = "strum", feature = "derive_more"))]
    pub(crate) const fn enum_variant_predicates(&self) -> Option<PredicateProvider> {
        self.enum_variant_predicates
    }

    /// Returns the configured provider for flat unit-enum parsing.
    #[cfg(feature = "strum")]
    pub(crate) const fn enum_string_parsing(&self) -> Option<StringParserProvider> {
        self.enum_string_parsing
    }

    /// Returns the configured provider for enum `Display` implementations.
    #[cfg(feature = "strum")]
    pub(crate) const fn enum_display(&self) -> Option<DisplayProvider> {
        self.enum_display
    }
}

#[cfg(test)]
mod tests {
    use super::DeriveResolutionConfig;
    #[cfg(feature = "strum")]
    use crate::rules::strum::utils::authored_contracts::{DisplayProvider, StringParserProvider};
    #[cfg(feature = "strum")]
    use crate::rules::strum::utils::enumeration::CollectionProvider;
    #[cfg(any(feature = "strum", feature = "derive_more"))]
    use crate::utils::variant_methods::PredicateProvider;

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

    #[cfg(feature = "strum")]
    #[test]
    fn parses_explicit_strum_predicate_provider() {
        let config = toml::from_str::<DeriveResolutionConfig>(
            r#"enum_variant_predicates = "strum_enum_is""#,
        )
        .expect("known predicate provider should parse");
        assert_eq!(
            config.enum_variant_predicates,
            Some(PredicateProvider::StrumEnumIs)
        );
    }

    #[cfg(feature = "strum")]
    #[test]
    fn rejects_implicit_predicate_provider_order() {
        assert!(
            toml::from_str::<DeriveResolutionConfig>(
                r#"enum_variant_predicates = "dependency_order""#
            )
            .is_err()
        );
    }

    #[cfg(feature = "strum")]
    #[test]
    fn parses_explicit_text_contract_providers() {
        let config = toml::from_str::<DeriveResolutionConfig>(
            r#"
                enum_display = "strum_display"
                enum_string_parsing = "derive_more_from_str"
            "#,
        )
        .expect("known text contract providers should parse");
        assert_eq!(config.enum_display, Some(DisplayProvider::StrumDisplay));
        assert_eq!(
            config.enum_string_parsing,
            Some(StringParserProvider::DeriveMoreFromStr)
        );
    }

    #[cfg(feature = "strum")]
    #[test]
    fn rejects_implicit_text_contract_provider_order() {
        assert!(
            toml::from_str::<DeriveResolutionConfig>(r#"enum_string_parsing = "dependency_order""#)
                .is_err()
        );
        assert!(
            toml::from_str::<DeriveResolutionConfig>(r#"enum_display = "dependency_order""#)
                .is_err()
        );
    }
}
