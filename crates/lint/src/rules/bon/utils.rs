extern crate rustc_ast;
extern crate rustc_lint;
extern crate rustc_span;

use rustc_ast::Attribute;
use rustc_lint::{EarlyContext, LintContext};
use rustc_span::{Span, Symbol};

pub(super) fn attribute_name(attribute: &Attribute) -> Option<Symbol> {
    attribute.path().last().copied()
}

pub(super) fn attribute_source(cx: &EarlyContext<'_>, attribute: &Attribute) -> Option<String> {
    cx.sess().source_map().span_to_snippet(attribute.span).ok()
}

pub(super) fn builder_attribute(attributes: &[Attribute]) -> Option<&Attribute> {
    attributes
        .iter()
        .find(|attribute| attribute_name(attribute).is_some_and(|name| name.as_str() == "builder"))
}

pub(super) fn plain_builder_attribute(
    cx: &EarlyContext<'_>,
    attributes: &[Attribute],
) -> Option<Span> {
    let attribute = builder_attribute(attributes)?;
    let source = attribute_source(cx, attribute)?;
    matches!(source.trim(), "#[builder]" | "#[bon::builder]").then_some(attribute.span)
}

pub(super) fn builder_attribute_contains(
    cx: &EarlyContext<'_>,
    attributes: &[Attribute],
    needle: &str,
) -> bool {
    builder_attribute(attributes)
        .and_then(|attribute| attribute_source(cx, attribute))
        .is_some_and(|source| source.contains(needle))
}

pub(super) fn has_attribute(attributes: &[Attribute], name: &str) -> bool {
    attributes
        .iter()
        .any(|attribute| attribute_name(attribute).is_some_and(|actual| actual.as_str() == name))
}

pub(super) fn derives_bon_builder(cx: &EarlyContext<'_>, attributes: &[Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        attribute_name(attribute).is_some_and(|name| name.as_str() == "derive")
            && attribute_source(cx, attribute).is_some_and(|source| source.contains("bon::Builder"))
    })
}

pub(super) fn is_option_type(ty: &str) -> bool {
    let compact: String = ty
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    compact.starts_with("Option<")
        || compact.starts_with("std::option::Option<")
        || compact.starts_with("core::option::Option<")
}

pub(super) fn configured_identifier(source: &str, key: &str) -> Option<String> {
    let tail = source.get(source.find(key)? + key.len()..)?.trim_start();
    let tail = tail.strip_prefix('=')?.trim_start();
    let tail = tail.strip_prefix('"').unwrap_or(tail);
    let value: String = tail
        .chars()
        .take_while(|character| character.is_alphanumeric() || *character == '_')
        .collect();
    (!value.is_empty()).then_some(value)
}
