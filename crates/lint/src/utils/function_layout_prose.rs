use convert_case::{Case, Casing};

use super::prose_case;

// -----------------------------------------------------------------------------
// FunctionLayoutProse: Phase comment prose normalization
// -----------------------------------------------------------------------------
/// Grammatical position used when normalizing one prose word.
#[derive(Clone, Copy)]
enum FunctionLayoutProsePosition {
    /// The word starts a sentence and receives sentence casing.
    SentenceStart,
    /// The word continues a sentence and receives lowercase casing.
    Continuation,
}

impl FunctionLayoutProsePosition {
    /// Advances the grammatical position after one authored word.
    fn after_word(word: &str) -> Self {
        if word.ends_with(['.', '!', '?']) {
            Self::SentenceStart
        } else {
            Self::Continuation
        }
    }
}

/// Validates and safely normalizes authored phase-comment prose.
pub struct FunctionLayoutProse;

impl FunctionLayoutProse {
    /// Validates presence, spacing, and sentence style of phase-comment prose.
    pub(crate) fn is_canonical(content: Option<&str>) -> bool {
        content.is_some_and(|content| content == content.trim() && Self::is_sentence_style(content))
    }

    /// Builds a canonical first-line replacement for safely repairable prose.
    pub(crate) fn replacement(content: Option<&str>, prefix: &str) -> Option<String> {
        let repairable = content
            .map(str::trim)
            .map(|content| content.trim_start_matches('-').trim_start())
            .filter(|content| !content.is_empty());
        repairable
            .and_then(Self::safe_sentence_replacement)
            .map(|content| format!("{prefix} {content}"))
    }

    /// Accepts populated conventional labels whose uppercase spelling is intentional.
    fn is_known_label(content: &str) -> bool {
        ["SAFETY:", "TODO:", "FIXME:", "NOTE:"].iter().any(|label| {
            content
                .strip_prefix(label)
                .is_some_and(|rest| rest.starts_with(' ') && !rest.trim().is_empty())
        })
    }

    /// Detects multiword all-uppercase prose rather than identifiers or acronyms.
    fn is_shouting(content: &str, words: &[&str]) -> bool {
        words.len() > 1
            && content
                .chars()
                .any(|character| character.is_ascii_alphabetic())
            && !content
                .chars()
                .any(|character| character.is_ascii_lowercase())
    }

    /// Returns whether casing normalization could corrupt code or a proper technical term.
    fn is_protected_word(word: &str) -> bool {
        let bare = word.trim_matches(|character: char| !character.is_ascii_alphanumeric());
        word.contains('`')
            || bare.contains(['_', '-'])
            || matches!(bare, "Rust" | "Rustfix")
            || bare.chars().skip(1).any(char::is_uppercase)
    }

    /// Normalizes one prose word while preserving protected authored spelling.
    fn normalize_word(word: &str, position: FunctionLayoutProsePosition) -> String {
        if Self::is_protected_word(word) {
            word.to_owned()
        } else if matches!(position, FunctionLayoutProsePosition::SentenceStart) {
            word.to_case(Case::Sentence)
        } else {
            word.to_case(Case::Lower)
        }
    }

    /// Returns whether an automatic whole-sentence case repair would be unsafe.
    fn has_protected_content(words: &[&str]) -> bool {
        words.iter().any(|word| {
            let bare = word.trim_matches(|character: char| !character.is_ascii_alphanumeric());
            word.contains(['`', '_'])
                || (bare.len() > 1 && bare.chars().all(|character| character.is_ascii_uppercase()))
        })
    }

    /// Validates natural sentence casing through protected word-by-word normalization.
    fn is_sentence_style(content: &str) -> bool {
        // Accept conventional labels before normalizing ordinary prose.
        if Self::is_known_label(content) {
            return true;
        }
        let words = content.split_whitespace().collect::<Vec<_>>();
        if words.is_empty() || Self::is_shouting(content, &words) {
            return false;
        }

        // Normalize prose while preserving code and established proper terms.
        let mut position = FunctionLayoutProsePosition::SentenceStart;
        let normalized_words = words.iter().map(|word| {
            let normalized = Self::normalize_word(word, position);
            position = FunctionLayoutProsePosition::after_word(word);
            normalized
        });

        // Compare authored prose with the casing crate's protected normalization.
        normalized_words.collect::<Vec<_>>().join(" ") == content
    }

    /// Produces a sentence-case repair only when no technical spelling is at risk.
    fn safe_sentence_replacement(content: &str) -> Option<String> {
        let words = content.split_whitespace().collect::<Vec<_>>();
        let is_shouting = Self::is_shouting(content, &words);
        let has_protected_content = !is_shouting && Self::has_protected_content(&words);
        (!has_protected_content).then(|| prose_case::sentence(content))
    }
}

// -----------------------------------------------------------------------------
// Tests: Unit tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::FunctionLayoutProse;

    #[test]
    fn accepts_natural_sentence_style_and_protected_terms() {
        assert!(FunctionLayoutProse::is_sentence_style(
            "Prepare the value using `u8` and HIR context."
        ));
        assert!(FunctionLayoutProse::is_sentence_style(
            "Read the input. Preserve Rust semantics."
        ));
        assert!(!FunctionLayoutProse::is_sentence_style(
            "Prepare The Value."
        ));
        assert!(!FunctionLayoutProse::is_sentence_style("THIS IS LOUD"));
    }

    #[test]
    fn accepts_only_populated_known_labels() {
        for label in ["SAFETY:", "TODO:", "FIXME:", "NOTE:"] {
            assert!(FunctionLayoutProse::is_known_label(&format!(
                "{label} Explain the exceptional case."
            )));
            assert!(!FunctionLayoutProse::is_known_label(label));
        }
    }

    #[test]
    fn suggests_only_unambiguous_case_repairs() {
        assert_eq!(
            FunctionLayoutProse::safe_sentence_replacement("THIS IS LOUD"),
            Some("This is loud".to_owned())
        );
        assert_eq!(
            FunctionLayoutProse::safe_sentence_replacement("Preserve HIR"),
            None
        );
        assert_eq!(
            FunctionLayoutProse::safe_sentence_replacement("Preserve `u8`"),
            None
        );
        assert_eq!(
            FunctionLayoutProse::replacement(Some("--- READ THE INPUT"), "//"),
            Some("// Read the input".to_owned())
        );
        assert_eq!(
            FunctionLayoutProse::replacement(Some("    read the input."), "//"),
            Some("// Read the input.".to_owned())
        );
    }
}
