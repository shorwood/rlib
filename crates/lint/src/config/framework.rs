#![cfg_attr(not(any(feature = "strum", feature = "thiserror")), allow(dead_code))]

use super::providers::{
    CollectionProvider, DisplayProvider, ErrorImplementationProvider,
    ErrorVariantConversionProvider, PredicateProvider, StringParserProvider,
};

/// Explicit provider selections for capabilities implemented by multiple derive frameworks.
#[derive(Clone, Default)]
pub struct DeriveResolutionConfig {
    /// Provider selected to implement `Error`.
    pub(super) error_implementation: Option<ErrorImplementationProvider>,
    /// Provider selected for error-variant conversions.
    pub(super) error_variant_conversion: Option<ErrorVariantConversionProvider>,
    /// Provider selected for enum collections.
    pub(super) enum_variant_collection: Option<CollectionProvider>,
    /// Provider selected for enum predicates.
    pub(super) enum_variant_predicates: Option<PredicateProvider>,
    /// Provider selected for enum string parsing.
    pub(super) enum_string_parsing: Option<StringParserProvider>,
    /// Provider selected for enum display.
    pub(super) enum_display: Option<DisplayProvider>,
}

impl DeriveResolutionConfig {
    /// Returns the configured error implementation provider.
    pub(crate) const fn error_implementation(&self) -> Option<ErrorImplementationProvider> {
        self.error_implementation
    }

    /// Returns the configured error-variant conversion provider.
    pub(crate) const fn error_variant_conversion(&self) -> Option<ErrorVariantConversionProvider> {
        self.error_variant_conversion
    }

    /// Returns the configured enum collection provider.
    pub(crate) const fn enum_variant_collection(&self) -> Option<CollectionProvider> {
        self.enum_variant_collection
    }

    /// Returns the configured enum predicate provider.
    pub(crate) const fn enum_variant_predicates(&self) -> Option<PredicateProvider> {
        self.enum_variant_predicates
    }

    /// Returns the configured enum string parser provider.
    pub(crate) const fn enum_string_parsing(&self) -> Option<StringParserProvider> {
        self.enum_string_parsing
    }

    /// Returns the configured enum display provider.
    pub(crate) const fn enum_display(&self) -> Option<DisplayProvider> {
        self.enum_display
    }
}
