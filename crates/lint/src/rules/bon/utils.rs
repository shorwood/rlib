extern crate rustc_ast;
extern crate rustc_lint;
extern crate rustc_span;

use rustc_ast::Attribute;
use rustc_lint::{EarlyContext, LintContext};
use rustc_span::{Span, Symbol};

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
        let attribute = builder_attribute(attributes)?;
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
        let Some(attribute) = builder_attribute(attributes) else {
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
}

/// Performs the `builder_attribute` step of the lint analysis.
pub(super) fn builder_attribute(attributes: &[Attribute]) -> Option<&Attribute> {
    attributes.iter().find(|attribute| {
        BonAttributeAnalysis::name(attribute).is_some_and(|name| name.as_str() == "builder")
    })
}

/// Performs the `has_attribute` step of the lint analysis.
pub(super) fn has_attribute(attributes: &[Attribute], name: &str) -> bool {
    attributes.iter().any(|attribute| {
        BonAttributeAnalysis::name(attribute).is_some_and(|actual| actual.as_str() == name)
    })
}

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
        // Prepare the values used by this stage.
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

        // Return the completed analysis result.
        (!value.is_empty()).then_some(value)
    }
}

/// Performs the `is_option_type` step of the lint analysis.
pub(super) fn is_option_type(ty: &str) -> bool {
    let compact: String = ty
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    compact.starts_with("Option<")
        || compact.starts_with("std::option::Option<")
        || compact.starts_with("core::option::Option<")
}
