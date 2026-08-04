use convert_case::{Case, Casing};

// -----------------------------------------------------------------------------
// Identifier: Rust identifier casing
// -----------------------------------------------------------------------------

/// Splits a Rust-style `PascalCase` identifier into normalized `PascalCase` words.
pub(crate) fn identifier_pascal_words(value: &str) -> Vec<String> {
    Case::Pascal
        .split(&value)
        .into_iter()
        .filter(|word| !word.is_empty())
        .map(|word| word.to_case(Case::Pascal))
        .collect()
}

/// Converts a Rust identifier from its authored convention to `PascalCase` words.
pub(crate) fn identifier_words(value: &str) -> Vec<String> {
    let source = if value.contains('_') {
        Case::Snake
    } else {
        Case::Pascal
    };
    source
        .split(&value)
        .into_iter()
        .filter(|word| !word.is_empty())
        .map(|word| word.to_case(Case::Pascal))
        .collect()
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
    (1..=first.len()).rev().find_map(|length| {
        names
            .iter()
            .skip(1)
            .all(|name| identifier_words(name).starts_with(&first[..length]))
            .then(|| first[..length].concat())
    })
}

// -----------------------------------------------------------------------------
// SentenceCase: Free-text sentence casing
// -----------------------------------------------------------------------------

/// Normalizes the first alphabetic character without rewriting the remaining text.
pub(crate) fn sentence_case(value: &str) -> String {
    let Some((index, character)) = value
        .char_indices()
        .find(|(_, character)| character.is_alphabetic())
    else {
        return value.to_owned();
    };
    let end = index + character.len_utf8();
    format!(
        "{}{}{}",
        &value[..index],
        character.to_string().to_case(Case::Upper),
        &value[end..]
    )
}

#[cfg(test)]
mod tests {
    use super::{
        identifier_is_pascal_case, identifier_longest_pascal_prefix, identifier_pascal_words,
        identifier_words,
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
}
