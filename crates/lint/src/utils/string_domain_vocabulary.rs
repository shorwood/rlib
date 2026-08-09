extern crate rustc_span;

use std::collections::HashSet;

use rustc_span::Symbol;

use super::identifier_case;
use super::parameter_analysis::Parameter;

// -----------------------------------------------------------------------------
// BehaviorVocabulary: Recognized domain operations
// -----------------------------------------------------------------------------

/// Domain behavior and invariant-establishment vocabulary.
struct BehaviorVocabulary;

impl BehaviorVocabulary {
    /// Operations conventionally owned by a parsed or validated domain type.
    const WORDS: &'static [&'static str] = &[
        "canonicalize",
        "check",
        "compare",
        "convert",
        "decode",
        "encode",
        "format",
        "normalize",
        "parse",
        "path",
        "render",
        "sanitize",
        "validate",
    ];

    /// Returns whether a word describes behavior conventionally owned by a domain type.
    fn contains(word: &str) -> bool {
        Self::WORDS.contains(&word)
    }

    /// Returns whether a word describes establishment of a domain invariant.
    fn establishes_invariant(word: &str) -> bool {
        matches!(
            word,
            "canonicalize" | "check" | "normalize" | "parse" | "sanitize" | "validate"
        )
    }
}

// -----------------------------------------------------------------------------
// DomainWords: Identifier derived domain vocabulary
// -----------------------------------------------------------------------------

/// Normalized semantic words from one Rust identifier.
struct DomainWords(
    /// Lowercase semantic words retained in authored order.
    Vec<String>,
);

impl DomainWords {
    /// Normalizes one Rust identifier into lowercase semantic words.
    fn normalized(identifier: Symbol) -> Self {
        let words = identifier_case::words(identifier.as_str());
        Self(words.into_iter().map(|word| word.to_lowercase()).collect())
    }

    /// Removes action and generic transport vocabulary from a domain candidate.
    fn meaningful(identifier: Symbol) -> Self {
        let words = Self::normalized(identifier).0.into_iter().filter(|word| {
            !BehaviorVocabulary::contains(word)
                && !matches!(
                    word.as_str(),
                    "contents"
                        | "input"
                        | "left"
                        | "message"
                        | "output"
                        | "raw"
                        | "right"
                        | "source"
                        | "text"
                        | "value"
                )
        });
        Self(words.collect())
    }

    /// Returns whether this identifier retains domain-specific vocabulary.
    const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Renders the retained lowercase words as one `PascalCase` type name.
    fn to_pascal(&self) -> String {
        self.0
            .iter()
            .map(|word| identifier_case::to_pascal(word))
            .collect()
    }

    /// Returns whether every retained word occurs in the operation vocabulary.
    fn is_supported_by(&self, operation: &HashSet<String>) -> bool {
        !self.is_empty() && self.0.iter().all(|word| operation.contains(word))
    }

    /// Converts these words into a set used for operation-name comparison.
    fn into_set(self) -> HashSet<String> {
        self.0.into_iter().collect()
    }
}

// -----------------------------------------------------------------------------
// StringDomainVocabulary: Public vocabulary queries
// -----------------------------------------------------------------------------

/// Returns whether an identifier contains behavior owned by a domain type.
pub(super) fn has_behavior(identifier: Symbol) -> bool {
    DomainWords::normalized(identifier)
        .0
        .iter()
        .any(|word| BehaviorVocabulary::contains(word))
}

/// Returns whether an identifier names an invariant-establishing operation.
pub(super) fn establishes_invariant(identifier: Symbol) -> bool {
    DomainWords::normalized(identifier)
        .0
        .iter()
        .any(|word| BehaviorVocabulary::establishes_invariant(word))
}

/// Infers a domain concept from a precise parameter or field name.
pub(super) fn from_parameter(identifier: Symbol) -> Option<String> {
    let words = DomainWords::meaningful(identifier);
    (!words.is_empty()).then(|| words.to_pascal())
}

/// Connects function vocabulary and textual parameter names to domain candidates.
pub(super) fn function_domains(function: Symbol, parameters: &[&Parameter]) -> HashSet<String> {
    // Derive the domain expressed directly by the operation name.
    let function_words = DomainWords::meaningful(function);
    let function_domain = (!function_words.is_empty()).then(|| function_words.to_pascal());
    let function_word_set = function_words.into_set();

    // Prefer domains supported by both the operation and a parameter name.
    let mut domains = HashSet::new();
    for parameter in parameters {
        let parameter_words = DomainWords::meaningful(parameter.name);
        if !parameter_words.is_supported_by(&function_word_set) {
            continue;
        }
        domains.insert(parameter_words.to_pascal());
    }

    // A unary behavior function may express its domain entirely in its name.
    if domains.is_empty()
        && parameters.len() == 1
        && let Some(domain) = function_domain
    {
        domains.insert(domain);
    }
    domains
}

#[cfg(test)]
mod tests {
    use super::rustc_span::{Symbol, create_default_session_globals_then};
    use super::{DomainWords, from_parameter};

    #[test]
    fn extracts_domain_names_from_precise_parameters() {
        create_default_session_globals_then(|| {
            assert_eq!(
                from_parameter(Symbol::intern("left_access_token")),
                Some("AccessToken".to_owned())
            );
            assert_eq!(from_parameter(Symbol::intern("source")), None);
        });
    }

    #[test]
    fn removes_behavior_and_transport_vocabulary() {
        create_default_session_globals_then(|| {
            assert_eq!(
                DomainWords::meaningful(Symbol::intern("normalize_project_slug")).0,
                ["project", "slug"]
            );
            assert!(DomainWords::meaningful(Symbol::intern("format_message")).is_empty());
        });
    }
}
