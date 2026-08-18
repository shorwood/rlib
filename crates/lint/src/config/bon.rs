#![cfg_attr(not(feature = "bon"), allow(dead_code))]

use std::collections::{HashMap, HashSet};

use super::schema::BonBaselineEntry;
use super::validation;

// -----------------------------------------------------------------------------
// BonMemberPath: Builder member lookup
// -----------------------------------------------------------------------------

/// Borrowed identity of one member on a named Bon builder.
#[derive(Clone, Copy)]
pub struct BonMemberPath<'name> {
    /// Builder type name.
    pub(crate) builder: &'name str,
    /// Required member name.
    pub(crate) member: &'name str,
}

// -----------------------------------------------------------------------------
// BonApiBaselineConfig: Published builder compatibility baseline
// -----------------------------------------------------------------------------

/// Validated published Bon builder members keyed by builder name.
#[derive(Clone, Default)]
pub struct BonApiBaselineConfig {
    /// Required members for each published builder.
    builders: HashMap<String, Vec<String>>,
}

impl TryFrom<Vec<BonBaselineEntry>> for BonApiBaselineConfig {
    type Error = String;

    fn try_from(entries: Vec<BonBaselineEntry>) -> Result<Self, Self::Error> {
        let mut builders = HashMap::new();
        for entry in entries {
            validation::name("bon-api-baseline.builder", &entry.builder)?;
            let mut members = HashSet::new();
            for member in &entry.members {
                validation::name("bon-api-baseline.members", member)?;

                // Repeating a member makes the compatibility baseline ambiguous.
                if !members.insert(member) {
                    return Err(format!(
                        "bon-api-baseline contains duplicate member `{member}` for builder `{}`",
                        entry.builder
                    ));
                }
            }

            // Builder names uniquely identify public builder contracts.
            if builders
                .insert(entry.builder.clone(), entry.members)
                .is_some()
            {
                return Err(format!(
                    "bon-api-baseline contains duplicate builder `{}`",
                    entry.builder
                ));
            }
        }
        Ok(Self { builders })
    }
}

impl BonApiBaselineConfig {
    /// Returns whether a builder already belongs to the published baseline.
    pub(crate) fn contains_builder(&self, builder: &str) -> bool {
        self.builders.contains_key(builder)
    }

    /// Returns whether a required builder member belongs to the baseline.
    pub(crate) fn contains_member(&self, path: BonMemberPath<'_>) -> bool {
        self.builders
            .get(path.builder)
            .is_some_and(|members| members.iter().any(|known| known == path.member))
    }
}
