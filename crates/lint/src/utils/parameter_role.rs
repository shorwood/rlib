use super::parameter_kind::ParameterKind;

// -----------------------------------------------------------------------------
// ParameterRole: Conventional and weak parameter vocabulary
// -----------------------------------------------------------------------------

/// Conventional role families that already communicate one shared domain.
const CONVENTIONAL_SETS: &[&[&str]] = &[
    &["left", "right"],
    &["lhs", "rhs"],
    &["first", "second"],
    &["new", "old"],
    &["source", "target"],
    &["end", "start"],
    &["max", "min"],
    &["lower", "upper"],
    &["x", "y"],
    &["x", "y", "z"],
    &["column", "row"],
    &["height", "width"],
    &["depth", "height", "width"],
];

/// Names describing ordinary text rather than a stable domain concept.
const GENERIC_TEXT_ROLES: &[&str] = &[
    "contents",
    "input",
    "label",
    "message",
    "pattern",
    "replacement",
    "source",
    "text",
    "value",
];

/// Conventional operations whose textual arguments are deliberately generic.
const GENERIC_TEXT_OPERATIONS: &[&str] = &["format", "join", "replace", "split", "write"];

/// Weak names that do not establish distinct semantic roles.
const WEAK_NAMES: &[&str] = &["a", "arg", "argument", "b", "item", "thing", "value"];

/// Returns whether a name is too generic to establish a distinct domain role.
pub(super) fn is_weak(name: &str) -> bool {
    WEAK_NAMES.contains(&name)
}

/// Returns whether a role belongs to ordinary text-transformation vocabulary.
fn is_generic(name: &str) -> bool {
    GENERIC_TEXT_ROLES.contains(&name) || matches!(name, "delimiter" | "separator")
}

/// Recognizes conventional role sets independently from compiler HIR records.
pub(super) fn names_are_conventional(
    function: &str,
    kind: ParameterKind,
    names: &[String],
) -> bool {
    // Accept established symmetric, coordinate, range, and dimension vocabularies.
    if CONVENTIONAL_SETS.iter().any(|roles| names == *roles) {
        return true;
    }
    if kind != ParameterKind::Text {
        return false;
    }

    // Recognize ordinary text roles independently from operation names.
    let all_roles_are_generic = names
        .iter()
        .all(|name| GENERIC_TEXT_ROLES.contains(&name.as_str()));

    // Recognize generic roles attached to conventional text transformations.
    let operation_is_generic = GENERIC_TEXT_OPERATIONS
        .iter()
        .any(|operation| function.contains(operation));
    let operation_roles_are_generic = names.iter().all(|name| is_generic(name));
    all_roles_are_generic || (operation_is_generic && operation_roles_are_generic)
}

#[cfg(test)]
mod tests {
    extern crate rustc_middle;

    use self::rustc_middle::ty;

    use super::{ParameterKind, names_are_conventional};

    #[test]
    fn recognizes_same_domain_parameter_roles() {
        assert!(names_are_conventional(
            "point",
            ParameterKind::Float {
                representation: ty::FloatTy::F64,
            },
            &["x".to_owned(), "y".to_owned()],
        ));
        assert!(names_are_conventional(
            "replace",
            ParameterKind::Text,
            &[
                "pattern".to_owned(),
                "replacement".to_owned(),
                "source".to_owned(),
            ],
        ));
    }

    #[test]
    fn retains_distinct_domain_roles() {
        assert!(!names_are_conventional(
            "authenticate",
            ParameterKind::Text,
            &[
                "organization".to_owned(),
                "token".to_owned(),
                "user".to_owned(),
            ],
        ));
    }
}
