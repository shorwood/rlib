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
    fn is_establishing_invariant(word: &str) -> bool {
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

    /// Removes action and generic transport vocabulary from a domain `candidate`.
    fn filtered_for_domain_inference(identifier: Symbol) -> Self {
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
// StringDomainSymbolExt: Public vocabulary queries
// -----------------------------------------------------------------------------

/// Domain-vocabulary queries colocated with compiler symbols.
pub(super) trait StringDomainSymbolExt {
    /// Returns whether this identifier contains behavior owned by a domain type.
    fn has_domain_behavior(self) -> bool;

    /// Returns whether this identifier names an invariant-establishing operation.
    fn has_invariant_establishing_name(self) -> bool;

    /// Infers a domain concept from this precise parameter or field name.
    fn parameter_domain(self) -> Option<String>;

    /// Connects this function name and textual parameters to domain candidates.
    fn function_domains(self, parameters: &[&Parameter]) -> HashSet<String>;
}

impl StringDomainSymbolExt for Symbol {
    fn has_domain_behavior(self) -> bool {
        DomainWords::normalized(self)
            .0
            .iter()
            .any(|word| BehaviorVocabulary::contains(word))
    }

    fn has_invariant_establishing_name(self) -> bool {
        DomainWords::normalized(self)
            .0
            .iter()
            .any(|word| BehaviorVocabulary::is_establishing_invariant(word))
    }

    fn parameter_domain(self) -> Option<String> {
        let words = DomainWords::filtered_for_domain_inference(self);
        (!words.is_empty()).then(|| words.to_pascal())
    }

    fn function_domains(self, parameters: &[&Parameter]) -> HashSet<String> {
        // Derive the domain expressed directly by the operation name.
        let function_words = DomainWords::filtered_for_domain_inference(self);
        let function_domain = (!function_words.is_empty()).then(|| function_words.to_pascal());
        let function_word_set = function_words.into_set();

        // Prefer domains supported by both the operation and a parameter name.
        let mut domains = HashSet::new();
        for parameter in parameters {
            let parameter_words = DomainWords::filtered_for_domain_inference(parameter.name);
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
}

// -----------------------------------------------------------------------------
// Tests: Unit tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::rustc_span::{Symbol, create_default_session_globals_then};
    use super::{DomainWords, StringDomainSymbolExt};

    #[test]
    fn extracts_domain_names_from_precise_parameters() {
        create_default_session_globals_then(|| {
            assert_eq!(
                Symbol::intern("left_access_token").parameter_domain(),
                Some("AccessToken".to_owned())
            );
            assert_eq!(Symbol::intern("source").parameter_domain(), None);
        });
    }

    #[test]
    fn removes_behavior_and_transport_vocabulary() {
        create_default_session_globals_then(|| {
            assert_eq!(
                DomainWords::filtered_for_domain_inference(Symbol::intern(
                    "normalize_project_slug"
                ))
                .0,
                ["project", "slug"]
            );
            assert!(
                DomainWords::filtered_for_domain_inference(Symbol::intern("format_message"))
                    .is_empty()
            );
        });
    }
}
