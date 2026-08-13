extern crate rustc_ast;
extern crate rustc_lint;
extern crate rustc_span;

use rustc_ast::Attribute;
use rustc_lint::{EarlyContext, LintContext};
use rustc_span::{Span, Symbol};

// -----------------------------------------------------------------------------
// BonAttributeAnalysis: Authored Bon attribute inspection
// -----------------------------------------------------------------------------

/// Owns source-level analysis of Bon-related attributes.
pub(super) struct BonAttributeAnalysis;

impl BonAttributeAnalysis {
    /// Returns the final path component of an attribute.
    pub(super) fn name(attribute: &Attribute) -> Option<Symbol> {
        attribute.path().last().copied()
    }

    /// Recovers the authored source for an attribute.
    pub(super) fn source(cx: &EarlyContext<'_>, attribute: &Attribute) -> Result<String, ()> {
        cx.sess()
            .source_map()
            .span_to_snippet(attribute.span)
            .map_err(|_error| ())
    }

    /// Returns a plain `builder` attribute with no additional policy.
    pub(super) fn plain_builder(cx: &EarlyContext<'_>, attributes: &[Attribute]) -> Option<Span> {
        let attribute = Self::builder(attributes)?;
        let source = match Self::source(cx, attribute) {
            Ok(source) => source,
            Err(_error) => return None,
        };
        matches!(source.trim(), "#[builder]" | "#[bon::builder]").then_some(attribute.span)
    }

    /// Returns whether a builder attribute contains an authored policy token.
    pub(super) fn builder_contains(
        cx: &EarlyContext<'_>,
        attributes: &[Attribute],
        needle: &str,
    ) -> bool {
        let Some(attribute) = Self::builder(attributes) else {
            return false;
        };
        Self::source(cx, attribute).is_ok_and(|source| source.contains(needle))
    }

    /// Returns whether the attributes derive `bon::Builder`.
    pub(super) fn derives_builder(cx: &EarlyContext<'_>, attributes: &[Attribute]) -> bool {
        attributes.iter().any(|attribute| {
            Self::name(attribute).is_some_and(|name| name.as_str() == "derive")
                && Self::source(cx, attribute).is_ok_and(|source| source.contains("bon::Builder"))
        })
    }

    /// Returns the first Bon builder attribute in a declaration.
    pub(super) fn builder(attributes: &[Attribute]) -> Option<&Attribute> {
        attributes
            .iter()
            .find(|attribute| Self::name(attribute).is_some_and(|name| name.as_str() == "builder"))
    }

    /// Returns whether a declaration carries an attribute with the requested final path name.
    pub(super) fn has(attributes: &[Attribute], name: &str) -> bool {
        attributes
            .iter()
            .any(|attribute| Self::name(attribute).is_some_and(|actual| actual.as_str() == name))
    }
}

// -----------------------------------------------------------------------------
// ConfiguredIdentifier: Named Bon option parsing
// -----------------------------------------------------------------------------

/// Authored attribute source and the named key to extract from it.
pub(super) struct ConfiguredIdentifier<'source> {
    /// Complete authored attribute source.
    pub(super) source: &'source str,
    /// Configuration key whose identifier value is requested.
    pub(super) key: &'source str,
}

impl ConfiguredIdentifier<'_> {
    /// Parses the configured identifier when the key has an identifier-like value.
    pub(super) fn parse(self) -> Option<String> {
        let tail = self
            .source
            .get(self.source.find(self.key)? + self.key.len()..)?
            .trim_start();
        let tail = tail.strip_prefix('=')?.trim_start();
        let tail = tail.strip_prefix('"').unwrap_or(tail);
        let value: String = tail
            .chars()
            .take_while(|character| character.is_alphanumeric() || *character == '_')
            .collect();

        (!value.is_empty()).then_some(value)
    }
}

// -----------------------------------------------------------------------------
// OptionType: Source-level optionality recognition
// -----------------------------------------------------------------------------

/// Source-level recognition for optional builder member types.
pub(super) struct OptionType;

impl OptionType {
    /// Recognizes common source spellings of `Option<T>`.
    pub(super) fn is_option(ty: &str) -> bool {
        let compact: String = ty
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();
        compact.starts_with("Option<")
            || compact.starts_with("std::option::Option<")
            || compact.starts_with("core::option::Option<")
    }
}
