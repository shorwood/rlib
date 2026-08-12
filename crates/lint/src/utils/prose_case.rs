use convert_case::{Case, Casing};

// -----------------------------------------------------------------------------
// ProseCase: Free text casing
// -----------------------------------------------------------------------------

/// Converts prose to canonical sentence case.
pub(super) fn sentence(value: &str) -> String {
    value.to_case(Case::Sentence)
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
    }
}
