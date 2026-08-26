use convert_case::{Case, Casing};

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

pub fn apply_case(value: &str, case: Option<&str>) -> String {
    match case {
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
