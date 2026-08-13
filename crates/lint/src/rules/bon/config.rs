use std::collections::HashMap;

use serde::Deserialize;

// -----------------------------------------------------------------------------
// BonApiBaselineConfig: Published builder compatibility
// -----------------------------------------------------------------------------

/// Published members retained for one builder in the compatibility baseline.
#[derive(Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
struct BonBuilderBaseline {
    /// Member names accepted by previously published call sequences.
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
/// Configured snapshot of previously published Bon builder members.
#[derive(Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct BonApiBaselineConfig {
    /// Builder snapshots indexed by public builder name.
    builders: HashMap<String, BonBuilderBaseline>,
}

impl BonApiBaselineConfig {
    /// Returns whether a builder existed in the published baseline.
    pub(super) fn contains_builder(&self, builder: &str) -> bool {
        self.builders.contains_key(builder)
    }

    /// Returns whether a member existed on its builder in the published baseline.
    pub(super) fn contains_member(&self, path: BonMemberPath<'_>) -> bool {
        self.builders
            .get(path.builder)
            .is_some_and(|baseline| baseline.members.iter().any(|known| known == path.member))
    }
}

// -----------------------------------------------------------------------------
// Tests: Bon compatibility baseline parsing
// -----------------------------------------------------------------------------

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
