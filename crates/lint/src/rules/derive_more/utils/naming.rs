use convert_case::{Case, Casing};

// -----------------------------------------------------------------------------
// Cases: Derive-more naming policies
// -----------------------------------------------------------------------------

/// Naming policies accepted by derive_more's container attributes.
pub const CASES: [&str; 8] = [
    "lowercase",
    "UPPERCASE",
    "PascalCase",
    "camelCase",
    "snake_case",
    "SCREAMING_SNAKE_CASE",
    "kebab-case",
    "SCREAMING-KEBAB-CASE",
];

// -----------------------------------------------------------------------------
// OptionalCaseExt: Applying an optional policy
// -----------------------------------------------------------------------------

/// Naming conversion behavior for an optional derive_more case policy.
pub trait OptionalCaseExt {
    /// Applies the configured policy to `value`, preserving it when no policy is recognized.
    fn apply_to(self, value: &str) -> String;
}

impl OptionalCaseExt for Option<&str> {
    fn apply_to(self, value: &str) -> String {
        match self {
            Some("lowercase") => value.to_case(Case::Flat),
            Some("UPPERCASE") => value.to_case(Case::UpperFlat),
            Some("PascalCase") => value.to_case(Case::Pascal),
            Some("camelCase") => value.to_case(Case::Camel),
            Some("snake_case") => value.to_case(Case::Snake),
            Some("SCREAMING_SNAKE_CASE") => value.to_case(Case::UpperSnake),
            Some("kebab-case") => value.to_case(Case::Kebab),
            Some("SCREAMING-KEBAB-CASE") => value.to_case(Case::UpperKebab),
            _ => value.to_owned(),
        }
    }
}
