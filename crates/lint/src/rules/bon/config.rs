use std::collections::HashMap;

use serde::Deserialize;

#[derive(Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct BonApiBaselineConfig {
    builders: HashMap<String, BonBuilderBaseline>,
}

#[derive(Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct BonBuilderBaseline {
    members: Vec<String>,
}

impl BonApiBaselineConfig {
    pub(crate) fn contains_builder(&self, builder: &str) -> bool {
        self.builders.contains_key(builder)
    }

    pub(crate) fn contains_member(&self, builder: &str, member: &str) -> bool {
        self.builders
            .get(builder)
            .is_some_and(|baseline| baseline.members.iter().any(|known| known == member))
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
        assert!(config.contains_member("Request", "host"));
        assert!(!config.contains_member("Request", "port"));
    }
}
