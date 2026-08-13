use std::collections::HashMap;

use serde::Deserialize;

#[derive(Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
/// Carries the `BonBuilderBaseline` state used by this analysis.
struct BonBuilderBaseline {
    /// Stores the `members` value used by this analysis.
    members: Vec<String>,
}

/// Builder and member identity used for baseline membership queries.
#[derive(Clone, Copy)]
pub(super) struct BonMemberPath<'name> {
    /// Builder type name.
    pub(super) builder: &'name str,
    /// Builder member name.
    pub(super) member: &'name str,
}
#[derive(Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
/// Carries the `BonApiBaselineConfig` state used by this analysis.
pub struct BonApiBaselineConfig {
    /// Stores the `builders` value used by this analysis.
    builders: HashMap<String, BonBuilderBaseline>,
}

impl BonApiBaselineConfig {
    /// Performs the `contains_builder` operation for this value.
    pub(super) fn contains_builder(&self, builder: &str) -> bool {
        self.builders.contains_key(builder)
    }

    /// Performs the `contains_member` operation for this value.
    pub(super) fn contains_member(&self, path: BonMemberPath<'_>) -> bool {
        self.builders
            .get(path.builder)
            .is_some_and(|baseline| baseline.members.iter().any(|known| known == path.member))
    }
}

#[cfg(test)]
mod tests {
    use super::BonApiBaselineConfig;

    #[test]
    fn parses_builder_member_snapshot() {
        let config = toml::from_str::<BonApiBaselineConfig>(
            r#"
                [builders.Request]
                members = ["host"]
            "#,
        )
        .expect("Bon API baseline should parse");
        assert!(config.contains_builder("Request"));
        assert!(config.contains_member(super::BonMemberPath {
            builder: "Request",
            member: "host",
        }));
        assert!(!config.contains_member(super::BonMemberPath {
            builder: "Request",
            member: "port",
        }));
    }
}
