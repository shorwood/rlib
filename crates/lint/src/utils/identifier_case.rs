use convert_case::{Case, Casing};

// -----------------------------------------------------------------------------
// Identifier: Rust identifier casing
// -----------------------------------------------------------------------------

/// Splits a Rust-style `PascalCase` identifier into normalized `PascalCase` words.
pub(crate) fn identifier_pascal_words(value: &str) -> Vec<String> {
    let words = Case::Pascal.split(&value);
    let populated = words.into_iter().filter(|word| !word.is_empty());
    populated.map(|word| word.to_case(Case::Pascal)).collect()
}

/// Converts a Rust identifier from its authored convention to `PascalCase` words.
pub(crate) fn identifier_words(value: &str) -> Vec<String> {
    let source = if value.contains('_') {
        Case::Snake
    } else {
        Case::Pascal
    };
    let words = source.split(&value);
    let populated = words.into_iter().filter(|word| !word.is_empty());
    populated.map(|word| word.to_case(Case::Pascal)).collect()
}

/// Normalizes a Rust identifier to canonical `PascalCase`.
pub(crate) fn identifier_pascal_case(value: &str) -> String {
    value.to_case(Case::Pascal)
}

/// Returns whether a string is already canonical `PascalCase`.
pub(crate) fn identifier_is_pascal_case(value: &str) -> bool {
    !value.is_empty() && identifier_pascal_case(value) == value
}

/// Returns the longest canonical `PascalCase` word prefix shared by the identifiers.
pub(crate) fn identifier_longest_pascal_prefix(names: &[&str]) -> Option<String> {
    let first = identifier_words(names.first()?);

    // Prefer the longest prefix that remains visible in every authored name.
    (1..=first.len()).rev().find_map(|length| {
        names
            .iter()
            .skip(1)
            .all(|name| identifier_words(name).starts_with(&first[..length]))
            .then(|| first[..length].concat())
    })
}

// -----------------------------------------------------------------------------
// SentenceCase: Free text sentence casing
// -----------------------------------------------------------------------------

/// Normalizes free text to canonical sentence case.
pub(crate) fn sentence_case(value: &str) -> String {
    value.to_case(Case::Sentence)
}

#[cfg(test)]
mod tests {
    use super::{
        identifier_is_pascal_case, identifier_longest_pascal_prefix, identifier_pascal_words,
        identifier_words, sentence_case,
    };

    #[test]
    fn understands_rust_identifier_boundaries() {
        assert_eq!(
            identifier_pascal_words("HttpServerConfig"),
            ["Http", "Server", "Config"]
        );
        assert_eq!(
            identifier_words("http_server_config"),
            ["Http", "Server", "Config"]
        );
    }

    #[test]
    fn validates_canonical_pascal_case() {
        assert!(identifier_is_pascal_case("HttpServerConfig"));
        assert!(!identifier_is_pascal_case("HTTPServerConfig"));
        assert!(!identifier_is_pascal_case("http_server_config"));
    }

    #[test]
    fn finds_prefixes_across_rust_identifier_conventions() {
        assert_eq!(
            identifier_longest_pascal_prefix(&["request_parser", "RequestPolicy"]),
            Some("Request".to_owned())
        );
    }

    #[test]
    fn normalizes_the_complete_sentence() {
        assert_eq!(sentence_case("THIS IS LOUD"), "This is loud");
        assert_eq!(
            sentence_case("already sentence case"),
            "Already sentence case"
        );
    }
}
