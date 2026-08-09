use super::identifier_case::{to_upper_snake, words};
use super::policy_literal_kind::PolicyCategory;

// -----------------------------------------------------------------------------
// PolicyNameSuggestion: Confidence gated constant names
// -----------------------------------------------------------------------------

/// Returns whether a type name explicitly advertises configuration ownership.
pub fn is_configuration_type_name(name: &str) -> bool {
    // Limit structural inference to conventional configuration container suffixes.
    ["Config", "Options", "Policy", "Settings", "Limits"]
        .iter()
        .any(|suffix| name.ends_with(suffix))
}

/// Derives a constant name only when authored vocabulary already describes the policy.
pub fn for_authored_name(name: &str, category: PolicyCategory) -> Option<String> {
    // Normalize the candidate before requiring an explicit policy-role word.
    let name = name.trim_start_matches('_');
    let canonical_words = words(name);
    if canonical_words.is_empty() {
        return None;
    }

    // Reject mechanically uppercased names that would not explain the policy role.
    let normalized = canonical_words
        .iter()
        .map(|word| word.to_ascii_lowercase())
        .collect::<Vec<_>>();
    if !normalized.iter().any(|word| category.matches_word(word)) {
        return None;
    }
    Some(to_upper_snake(name))
}

/// Authored configuration vocabulary used to infer one constant name.
pub struct ConfigurationNameContext<'name> {
    /// Name of the configuration type that supplies domain context.
    pub(crate) type_name: &'name str,
    /// Name of the field that supplies the policy role.
    pub(crate) field_name: &'name str,
}

impl ConfigurationNameContext<'_> {
    /// Returns a field-derived name only when it already includes non-policy vocabulary.
    fn precise_field_suggestion(&self, category: PolicyCategory) -> Option<String> {
        let suggestion = for_authored_name(self.field_name, category)?;
        let has_domain_word = words(self.field_name).into_iter().any(|word| {
            let word = word.to_ascii_lowercase();
            !PolicyCategory::all()
                .into_iter()
                .any(|candidate| candidate.matches_word(&word))
        });
        has_domain_word.then_some(suggestion)
    }

    /// Extracts domain words while discarding configuration container suffixes.
    fn domain_words(&self) -> Vec<String> {
        words(self.type_name)
            .into_iter()
            .filter(|word| {
                !matches!(
                    word.as_str(),
                    "Config" | "Options" | "Policy" | "Settings" | "Limits"
                )
            })
            .collect()
    }

    /// Combines domain and field vocabulary without displacing leading bounds.
    fn combined_name(&self, domain: &str) -> String {
        // Keep `max` and `min` as leading qualifiers in the resulting constant name.
        let maximum_remainder = self.field_name.strip_prefix("max_");
        let minimum_remainder = self.field_name.strip_prefix("min_");
        match (maximum_remainder, minimum_remainder) {
            (Some(remainder), _) => format!("max_{domain}_{remainder}"),
            (_, Some(remainder)) => format!("min_{domain}_{remainder}"),
            (None, None) => format!("{domain}_{}", self.field_name),
        }
    }

    /// Combines a policy field with its configuration type when domain context is missing.
    pub(crate) fn infer(self, category: PolicyCategory) -> Option<String> {
        // Preserve precise field vocabulary before consulting the surrounding type.
        if let Some(suggestion) = self.precise_field_suggestion(category) {
            return Some(suggestion);
        }

        // Recover domain vocabulary and keep boundary qualifiers in canonical order.
        let domain = self.domain_words();
        if domain.is_empty() {
            return None;
        }
        let combined = self.combined_name(&domain.concat());
        for_authored_name(&combined, category)
    }
}

#[cfg(test)]
mod tests {
    use super::{ConfigurationNameContext, for_authored_name};
    use crate::utils::policy_literal_kind::PolicyCategory;

    #[test]
    fn preserves_precise_authored_policy_vocabulary() {
        assert_eq!(
            for_authored_name("max_delivery_attempts", PolicyCategory::Retry),
            Some("MAX_DELIVERY_ATTEMPTS".to_owned())
        );
        assert_eq!(for_authored_name("value", PolicyCategory::Threshold), None);
    }

    #[test]
    fn adds_configuration_domain_when_the_field_is_generic() {
        let context = ConfigurationNameContext {
            type_name: "DeliveryPolicy",
            field_name: "timeout",
        };
        assert_eq!(
            context.infer(PolicyCategory::Timing),
            Some("DELIVERY_TIMEOUT".to_owned())
        );
    }
}
