use convert_case::{Case, Casing};

// -----------------------------------------------------------------------------
// IdentifierWords: Canonical identifier words
// -----------------------------------------------------------------------------

/// Normalized identifier word operations shared by public casing helpers.
struct IdentifierWords;

impl IdentifierWords {
    /// Splits and normalizes identifier text according to its source convention.
    fn from_case(value: &str, source: Case<'_>) -> Vec<String> {
        let words = source.split(&value);
        let populated = words.into_iter().filter(|word| !word.is_empty());
        populated.map(|word| word.to_case(Case::Pascal)).collect()
    }
}

// -----------------------------------------------------------------------------
// IdentifierCase: Rust identifier casing
// -----------------------------------------------------------------------------

/// Converts identifier text to canonical `PascalCase`.
pub(super) fn to_pascal(value: &str) -> String {
    value.to_case(Case::Pascal)
}

/// Converts identifier text to canonical `UPPER_SNAKE_CASE`.
pub(super) fn to_upper_snake(value: &str) -> String {
    value.to_case(Case::UpperSnake)
}

/// Checks whether identifier text is canonical `snake_case`.
pub(super) fn is_snake(value: &str) -> bool {
    !value.is_empty() && value.to_case(Case::Snake) == value
}

/// Checks whether identifier text is canonical `PascalCase`.
pub(super) fn is_pascal(value: &str) -> bool {
    !value.is_empty() && to_pascal(value) == value
}

/// Splits an authored Rust identifier into canonical `PascalCase` words.
pub(super) fn words(value: &str) -> Vec<String> {
    let source = if value.contains('_') {
        Case::Snake
    } else {
        Case::Pascal
    };
    IdentifierWords::from_case(value, source)
}

/// Splits a known `PascalCase` identifier into canonical words.
pub(super) fn pascal_words(value: &str) -> Vec<String> {
    IdentifierWords::from_case(value, Case::Pascal)
}

// -----------------------------------------------------------------------------
// LongestCommonPascalPrefix: Shared identifier families
// -----------------------------------------------------------------------------

/// Finds the longest shared canonical `PascalCase` word prefix.
pub(super) fn longest_common_pascal_prefix(names: &[&str]) -> Option<String> {
    let mut names = names.iter();
    let mut prefix = words(names.next()?);

    // Shorten the candidate prefix against each remaining identifier.
    for name in names {
        // Measure the common leading word sequence before truncating the candidate.
        let candidate = words(name);
        let paired_words = prefix.iter().zip(&candidate);
        let shared = paired_words
            .take_while(|(left, right)| left == right)
            .count();
        prefix.truncate(shared);

        // Stop immediately when no canonical family prefix survives.
        if !prefix.is_empty() {
            continue;
        }
        return None;
    }

    // Render the surviving canonical word sequence as one type-family prefix.
    Some(prefix.concat())
}

#[cfg(test)]
mod tests {
    use super::{
        is_pascal, is_snake, longest_common_pascal_prefix, pascal_words, to_upper_snake, words,
    };

    #[test]
    fn understands_rust_identifier_boundaries() {
        assert_eq!(
            pascal_words("HttpServerConfig"),
            ["Http", "Server", "Config"]
        );
        assert_eq!(words("http_server_config"), ["Http", "Server", "Config"]);
    }

    #[test]
    fn validates_canonical_pascal_case() {
        assert!(is_pascal("HttpServerConfig"));
        assert!(!is_pascal("HTTPServerConfig"));
        assert!(!is_pascal("http_server_config"));
    }

    #[test]
    fn validates_canonical_snake_case() {
        assert!(is_snake("http_server_config"));
        assert!(!is_snake("HttpServerConfig"));
    }

    #[test]
    fn renders_constant_identifiers() {
        assert_eq!(to_upper_snake("deliveryRetryLimit"), "DELIVERY_RETRY_LIMIT");
    }

    #[test]
    fn finds_prefixes_across_rust_identifier_conventions() {
        assert_eq!(
            longest_common_pascal_prefix(&["request_parser", "RequestPolicy"]),
            Some("Request".to_owned())
        );
    }
}
