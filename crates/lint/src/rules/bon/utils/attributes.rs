extern crate rustc_ast;
extern crate rustc_lint;
extern crate rustc_span;

use rustc_ast::Attribute;
use rustc_lint::{EarlyContext, LintContext};
use rustc_span::{Span, Symbol};

/// Canonical top-level Bon builder option name.
#[derive(Clone, Copy)]
pub struct BuilderOption(
    /// Authored option spelling.
    &'static str,
);

impl BuilderOption {
    /// Conversion through `Into`.
    pub const INTO: Self = Self("into");

    /// Custom conversion closure.
    pub const WITH: Self = Self("with");

    /// Generated builder entry-point name.
    pub const START_FN: Self = Self("start_fn");

    /// Generated builder finishing-function name.
    pub const FINISH_FN: Self = Self("finish_fn");

    /// Defaulted member policy.
    pub const DEFAULT: Self = Self("default");

    /// Skipped setter policy.
    pub const SKIP: Self = Self("skip");

    /// Custom stored-field policy.
    pub const FIELD: Self = Self("field");

    /// Explicitly required member policy.
    pub const REQUIRED: Self = Self("required");

    /// Returns the authored option spelling.
    const fn as_str(self) -> &'static str {
        self.0
    }
}

// -----------------------------------------------------------------------------
// CommentMarkerBytes: Rust comment delimiter width
// -----------------------------------------------------------------------------

/// Byte width of each Rust comment delimiter inspected below.
const COMMENT_MARKER_BYTES: usize = 2;

// -----------------------------------------------------------------------------
// BonAttributeAnalysis: Authored Bon attribute inspection
// -----------------------------------------------------------------------------

/// Owns source-level analysis of Bon-related attributes.
pub struct BonAttributeAnalysis;

impl BonAttributeAnalysis {
    /// Returns the final path component of an attribute.
    pub fn name(attribute: &Attribute) -> Option<Symbol> {
        attribute.path().last().copied()
    }

    /// Recovers the authored source for an attribute.
    pub fn source(cx: &EarlyContext<'_>, attribute: &Attribute) -> Result<String, ()> {
        cx.sess()
            .source_map()
            .span_to_snippet(attribute.span)
            .map_err(|_error| ())
    }

    /// Returns a plain `builder` attribute with no additional policy.
    pub fn plain_builder(cx: &EarlyContext<'_>, attributes: &[Attribute]) -> Option<Span> {
        let attribute = Self::builder(attributes)?;
        let source = match Self::source(cx, attribute) {
            Ok(source) => source,
            // Unavailable authored text cannot prove that the attribute is plain.
            Err(_error) => return None,
        };
        matches!(source.trim(), "#[builder]" | "#[bon::builder]").then_some(attribute.span)
    }

    /// Returns whether a builder attribute contains an authored policy token.
    pub fn contains_builder_value(
        cx: &EarlyContext<'_>,
        attributes: &[Attribute],
        needle: &str,
    ) -> bool {
        // A declaration without a builder attribute contains no builder policy.
        let Some(attribute) = Self::builder(attributes) else {
            return false;
        };
        Self::source(cx, attribute).is_ok_and(|source| source.contains(needle))
    }

    /// Returns whether a builder attribute contains a top-level option with this exact name.
    pub fn has_builder_option(
        cx: &EarlyContext<'_>,
        attributes: &[Attribute],
        expected: BuilderOption,
    ) -> bool {
        // A declaration without a builder attribute has no configured option.
        let Some(attribute) = Self::builder(attributes) else {
            return false;
        };

        // Missing authored attribute text prevents top-level option parsing.
        let Ok(source) = Self::source(cx, attribute) else {
            return false;
        };

        // An attribute without an option payload contains no top-level option.
        let Some(open) = source.find('(') else {
            return false;
        };

        // An unterminated payload cannot be parsed as a complete option list.
        let Some(close) = source.rfind(')') else {
            return false;
        };
        let payload = &source[open + 1..close];
        let mut nesting = 0_usize;
        let mut start = 0_usize;
        for (index, character) in payload.char_indices() {
            match character {
                '(' | '[' | '{' => nesting += 1,
                ')' | ']' | '}' => nesting = nesting.saturating_sub(1),
                ',' if nesting == 0 => {
                    // A matching complete segment proves the requested option immediately.
                    if Self::option_name(&payload[start..index]) == expected.as_str() {
                        return true;
                    }
                    start = index + character.len_utf8();
                }
                _ => {}
            }
        }
        Self::option_name(&payload[start..]) == expected.as_str()
    }

    /// Returns whether a top-level builder option is present without a value or payload.
    pub fn has_builder_bare_option(
        cx: &EarlyContext<'_>,
        attributes: &[Attribute],
        expected: BuilderOption,
    ) -> bool {
        // A declaration without a builder attribute has no bare option.
        let Some(attribute) = Self::builder(attributes) else {
            return false;
        };

        // Missing authored attribute text prevents top-level option parsing.
        let Ok(source) = Self::source(cx, attribute) else {
            return false;
        };

        // An attribute without an option payload contains no bare option.
        let Some(open) = source.find('(') else {
            return false;
        };

        // An unterminated payload cannot be parsed as a complete option list.
        let Some(close) = source.rfind(')') else {
            return false;
        };
        let payload = &source[open + 1..close];
        let mut nesting = 0_usize;
        let mut start = 0_usize;
        for (index, character) in payload.char_indices() {
            match character {
                '(' | '[' | '{' => nesting += 1,
                ')' | ']' | '}' => nesting = nesting.saturating_sub(1),
                ',' if nesting == 0 => {
                    // A matching valueless segment proves the requested bare option immediately.
                    if Self::is_bare_option(&payload[start..index], expected) {
                        return true;
                    }
                    start = index + character.len_utf8();
                }
                _ => {}
            }
        }
        Self::is_bare_option(&payload[start..], expected)
    }

    /// Returns whether the attributes derive `bon::Builder`.
    pub fn has_builder_derive(cx: &EarlyContext<'_>, attributes: &[Attribute]) -> bool {
        attributes.iter().any(|attribute| {
            Self::name(attribute).is_some_and(|name| name.as_str() == "derive")
                && Self::source(cx, attribute).is_ok_and(|source| source.contains("bon::Builder"))
        })
    }

    /// Returns the first Bon builder attribute in a declaration.
    pub fn builder(attributes: &[Attribute]) -> Option<&Attribute> {
        attributes
            .iter()
            .find(|attribute| Self::name(attribute).is_some_and(|name| name.as_str() == "builder"))
    }

    /// Returns whether a declaration carries an attribute with the requested final path name.
    pub fn has_attribute(attributes: &[Attribute], name: &str) -> bool {
        attributes
            .iter()
            .any(|attribute| Self::name(attribute).is_some_and(|actual| actual.as_str() == name))
    }

    /// Checks one top-level option segment after removing authored comments.
    fn is_bare_option(segment: &str, expected: BuilderOption) -> bool {
        let uncommented = Self::without_comments(segment);
        let segment = uncommented.trim();
        let expected = expected.as_str();
        Self::option_name(segment) == expected && segment[expected.len()..].trim().is_empty()
    }

    /// Removes line and block comments from a short attribute option segment.
    fn without_comments(source: &str) -> String {
        let bytes = source.as_bytes();
        let mut result = String::with_capacity(source.len());
        let mut index = 0_usize;
        let mut block_depth = 0_usize;
        while index < bytes.len() {
            if block_depth > 0 {
                if bytes.get(index..index + COMMENT_MARKER_BYTES) == Some(b"/*") {
                    block_depth += 1;
                    index += COMMENT_MARKER_BYTES;
                } else if bytes.get(index..index + COMMENT_MARKER_BYTES) == Some(b"*/") {
                    block_depth -= 1;
                    index += COMMENT_MARKER_BYTES;
                } else {
                    index += 1;
                }
                continue;
            }
            if bytes.get(index..index + COMMENT_MARKER_BYTES) == Some(b"/*") {
                block_depth = 1;
                index += COMMENT_MARKER_BYTES;
                continue;
            }
            if bytes.get(index..index + COMMENT_MARKER_BYTES) == Some(b"//") {
                break;
            }
            let character = source[index..]
                .chars()
                .next()
                .expect("the byte index is inside the source");
            result.push(character);
            index += character.len_utf8();
        }
        result
    }

    /// Returns the leading identifier of one top-level builder option.
    fn option_name(segment: &str) -> &str {
        let segment = segment.trim_start();
        let end = segment
            .find(|character: char| !character.is_alphanumeric() && character != '_')
            .unwrap_or(segment.len());
        &segment[..end]
    }
}

// -----------------------------------------------------------------------------
// ConfiguredIdentifier: Named Bon option parsing
// -----------------------------------------------------------------------------

/// Authored attribute source and the named key to extract from it.
pub struct ConfiguredIdentifier<'source> {
    /// Complete authored attribute source.
    pub source: &'source str,
    /// Configuration key whose identifier value is requested.
    pub key: &'source str,
}

impl ConfiguredIdentifier<'_> {
    /// Parses the configured identifier when the key has an identifier-like value.
    pub fn parse(self) -> Option<String> {
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
pub struct OptionType;

impl OptionType {
    /// Recognizes common source spellings of `Option<T>`.
    pub fn is_option(ty: &str) -> bool {
        let compact: String = ty
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();
        compact.starts_with("Option<")
            || compact.starts_with("std::option::Option<")
            || compact.starts_with("core::option::Option<")
    }
}
