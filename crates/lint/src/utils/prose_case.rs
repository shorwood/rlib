use convert_case::{Case, Casing};

// -----------------------------------------------------------------------------
// ProseCase: Sentence normalization
// -----------------------------------------------------------------------------

/// Capitalizes prose without rewriting authored proper nouns or acronyms.
pub(super) fn sentence(value: &str) -> String {
    let is_shouting = value
        .chars()
        .filter(|character| character.is_alphabetic())
        .all(char::is_uppercase);

    // All-uppercase prose needs whole-string normalization rather than first-letter casing.
    if is_shouting {
        return value.to_case(Case::Sentence);
    }

    // Prose without alphabetic characters has no sentence initial to normalize.
    let Some((index, first)) = value
        .char_indices()
        .find(|(_, character)| character.is_alphabetic())
    else {
        return value.to_owned();
    };
    let mut normalized = String::with_capacity(value.len());
    normalized.push_str(&value[..index]);
    normalized.extend(first.to_uppercase());
    normalized.push_str(&value[index + first.len_utf8()..]);
    normalized
}

// -----------------------------------------------------------------------------
// Tests: Unit tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::sentence;

    #[test]
    fn normalizes_the_complete_sentence() {
        assert_eq!(sentence("THIS IS LOUD"), "This is loud");
        assert_eq!(sentence("already sentence case"), "Already sentence case");
        assert_eq!(
            sentence("authored Bon API policy"),
            "Authored Bon API policy"
        );
        assert_eq!(
            sentence("policy-bearing source-level contract"),
            "Policy-bearing source-level contract"
        );
    }
}
