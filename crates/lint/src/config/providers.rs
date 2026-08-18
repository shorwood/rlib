use serde::Deserialize;

// -----------------------------------------------------------------------------
// ErrorVariantConversionProvider: Error conversion ownership
// -----------------------------------------------------------------------------

/// Framework selected to generate conversions into error variants.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorVariantConversionProvider {
    /// Derive More's `From` derive.
    DeriveMoreFrom,
    /// thiserror's `#[from]` field contract.
    ThiserrorFrom,
}

// -----------------------------------------------------------------------------
// ErrorImplementationProvider: Error implementation ownership
// -----------------------------------------------------------------------------

/// Framework selected to generate an `Error` implementation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorImplementationProvider {
    /// Derive More's `Error` derive.
    DeriveMoreError,
    /// thiserror's `Error` derive.
    ThiserrorError,
}

// -----------------------------------------------------------------------------
// CollectionProvider: Enum collection ownership
// -----------------------------------------------------------------------------

/// Framework surface selected to generate an enum collection.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CollectionProvider {
    /// Strum's `EnumIter` derive.
    StrumEnumIter,
    /// Strum's `VariantArray` derive.
    StrumVariantArray,
}

// -----------------------------------------------------------------------------
// PredicateProvider: Enum predicate ownership
// -----------------------------------------------------------------------------

/// Framework selected to generate enum variant predicates.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PredicateProvider {
    /// Strum's `EnumIs` derive.
    StrumEnumIs,
    /// Derive More's `IsVariant` derive.
    DeriveMoreIsVariant,
}

// -----------------------------------------------------------------------------
// StringParserProvider: Enum parser ownership
// -----------------------------------------------------------------------------

/// Framework selected to generate enum string parsing.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum StringParserProvider {
    /// Strum's `EnumString` derive.
    StrumEnumString,
    /// Derive More's `FromStr` derive.
    DeriveMoreFromStr,
}

// -----------------------------------------------------------------------------
// DisplayProvider: Enum display ownership
// -----------------------------------------------------------------------------

/// Framework selected to generate enum display output.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DisplayProvider {
    /// Strum's `Display` derive.
    StrumDisplay,
    /// Derive More's `Display` derive.
    DeriveMoreDisplay,
}
