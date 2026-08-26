extern crate rustc_lint;
extern crate rustc_span;

use rustc_lint::{LateContext, LintContext};
use rustc_span::{BytePos, Span};

// -----------------------------------------------------------------------------
// CandidatePolicy: One legal container-wide naming rule
// -----------------------------------------------------------------------------

/// Effective member names produced by one legal container-wide naming rule.
pub struct CandidatePolicy {
    /// Attribute value naming the policy in the target derive crate.
    pub name: &'static str,
    /// Names produced for participating members in declaration order.
    pub names: Vec<String>,
    /// Additional directives required by this policy, such as Strum affixes.
    pub directive_cost: usize,
}

/// A uniquely determined container rule and the members that remain exceptions.
pub struct FactoredPolicy {
    /// Attribute value naming the selected policy.
    pub name: &'static str,
    /// Declaration-order indexes that still require leaf overrides.
    pub exceptions: Vec<usize>,
    /// Total number of directives in the factored representation.
    pub directive_cost: usize,
}

/// Selects a unique, strictly smaller container policy for an effective name mapping.
pub fn factor_names(
    effective_names: &[String],
    authored_directives: usize,
    candidates: impl IntoIterator<Item = CandidatePolicy>,
) -> Option<FactoredPolicy> {
    let required_exceptions = vec![false; effective_names.len()];
    factor_names_with_required(
        effective_names,
        authored_directives,
        &required_exceptions,
        candidates,
    )
}

/// Selects a container policy while retaining members whose leaf directives carry extra behavior.
pub fn factor_names_with_required(
    effective_names: &[String],
    authored_directives: usize,
    required_exceptions: &[bool],
    candidates: impl IntoIterator<Item = CandidatePolicy>,
) -> Option<FactoredPolicy> {
    if required_exceptions.len() != effective_names.len() {
        return None;
    }
    let mut best: Option<FactoredPolicy> = None;
    let mut ambiguous = false;

    for candidate in candidates {
        if candidate.names.len() != effective_names.len() {
            continue;
        }
        let exceptions = candidate
            .names
            .iter()
            .zip(effective_names)
            .enumerate()
            .filter_map(|(index, (candidate, effective))| {
                (required_exceptions[index] || candidate != effective).then_some(index)
            })
            .collect::<Vec<_>>();
        let directive_cost = candidate.directive_cost + exceptions.len();
        if directive_cost >= authored_directives {
            continue;
        }
        let policy = FactoredPolicy {
            name: candidate.name,
            exceptions,
            directive_cost,
        };
        match &best {
            None => {
                best = Some(policy);
                ambiguous = false;
            }
            Some(current) if directive_cost < current.directive_cost => {
                best = Some(policy);
                ambiguous = false;
            }
            Some(current) if directive_cost == current.directive_cost => {
                // Equivalent present-day mappings can imply different policy for future members.
                ambiguous = true;
            }
            Some(_) => {}
        }
    }

    (!ambiguous).then_some(best).flatten()
}

/// Finds one complete, standalone attribute inside an authored source range.
pub fn standalone_attribute_span(
    cx: &LateContext<'_>,
    search_span: Span,
    expected_without_whitespace: &str,
) -> Option<Span> {
    let source = cx.sess().source_map().span_to_snippet(search_span).ok()?;
    let bytes = source.as_bytes();
    let mut matches = Vec::new();
    let mut cursor = 0;
    while cursor + 1 < bytes.len() {
        let Some(relative) = source[cursor..].find("#[") else {
            break;
        };
        let start = cursor + relative;
        let mut end = start + 2;
        let mut depth = 1_u32;
        while end < bytes.len() && depth > 0 {
            match bytes[end] {
                b'[' => depth += 1,
                b']' => depth -= 1,
                _ => {}
            }
            end += 1;
        }
        if depth != 0 {
            return None;
        }
        let normalized = source[start..end]
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>();
        if normalized == expected_without_whitespace {
            matches.push((start, end));
        }
        cursor = end;
    }
    let [(start, end)] = matches.as_slice() else {
        return None;
    };
    let start = u32::try_from(*start).ok()?;
    let end = u32::try_from(*end).ok()?;
    Some(
        search_span
            .with_lo(search_span.lo() + BytePos(start))
            .with_hi(search_span.lo() + BytePos(end)),
    )
}

#[cfg(test)]
mod tests {
    use super::{CandidatePolicy, factor_names, factor_names_with_required};

    #[test]
    fn selects_a_smaller_default_with_an_exception() {
        let effective = ["firstValue", "secondValue", "legacy"].map(str::to_owned);
        let policy = factor_names(
            &effective,
            3,
            [CandidatePolicy {
                name: "camelCase",
                names: ["firstValue", "secondValue", "legacyValue"]
                    .map(str::to_owned)
                    .into(),
                directive_cost: 1,
            }],
        )
        .expect("container policy should reduce three leaf directives to a default and exception");

        assert_eq!(policy.name, "camelCase");
        assert_eq!(policy.exceptions, [2]);
        assert_eq!(policy.directive_cost, 2);
    }

    #[test]
    fn rejects_equal_cost_and_ambiguous_policies() {
        let effective = ["one", "two"].map(str::to_owned);
        assert!(
            factor_names(
                &effective,
                2,
                [CandidatePolicy {
                    name: "snake_case",
                    names: effective.to_vec(),
                    directive_cost: 2,
                }],
            )
            .is_none()
        );
        assert!(
            factor_names(
                &effective,
                3,
                ["lowercase", "snake_case"].map(|name| CandidatePolicy {
                    name,
                    names: effective.to_vec(),
                    directive_cost: 1,
                }),
            )
            .is_none()
        );
    }

    #[test]
    fn retains_semantically_required_leaf_directives() {
        let effective = ["firstValue", "secondValue", "thirdValue", "legacy"].map(str::to_owned);
        let policy = factor_names_with_required(
            &effective,
            4,
            &[true, false, false, false],
            [CandidatePolicy {
                name: "camelCase",
                names: ["firstValue", "secondValue", "thirdValue", "legacyValue"]
                    .map(str::to_owned)
                    .into(),
                directive_cost: 1,
            }],
        )
        .expect("the alias-bearing member and spelling exception should both remain");

        assert_eq!(policy.exceptions, [0, 3]);
    }
}
