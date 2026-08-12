extern crate rustc_hir;
extern crate rustc_middle;

use std::collections::HashSet;
use std::fmt;

use rustc_hir::def::DefKind;
use rustc_hir::def_id::{CRATE_DEF_ID, LocalDefId};
use rustc_middle::ty::TyCtxt;

// -----------------------------------------------------------------------------
// VisibilityBoundary: Canonical reach
// -----------------------------------------------------------------------------

/// Canonical source visibility ordered from narrowest to broadest.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum VisibilityBoundary {
    /// Reach limited to the defining module and its descendants.
    Private,
    /// Reach extended to the defining module's immediate parent subtree.
    Super,
    /// Reach extended across the current crate.
    Crate,
    /// Reach extended to downstream crates.
    Public,
}

impl VisibilityBoundary {
    /// Maps resolved use modules to the narrowest supported source visibility.
    pub(super) fn for_uses(
        tcx: TyCtxt<'_>,
        defining_module: LocalDefId,
        use_modules: impl IntoIterator<Item = LocalDefId>,
    ) -> Self {
        // Collapse every observed module into one semantic ancestor boundary.
        let common = use_modules
            .into_iter()
            .fold(defining_module, |common, module| {
                Self::lowest_common_module(tcx, common, module)
            });

        // Keep references owned by the defining module private.
        if common == defining_module {
            return Self::Private;
        }

        // Preserve only the immediate parent restriction from rust's path visibility lattice.
        let parent = tcx
            .opt_local_parent(defining_module)
            .filter(|parent| tcx.def_kind(*parent) == DefKind::Mod);
        if parent == Some(common) {
            Self::Super
        } else {
            Self::Crate
        }
    }

    /// Finds the nearest module containing both supplied modules.
    fn lowest_common_module(tcx: TyCtxt<'_>, left: LocalDefId, right: LocalDefId) -> LocalDefId {
        // Retain every ancestor of the left module for constant-time membership checks.
        let mut left_ancestors = HashSet::new();
        let mut cursor = Some(left);
        while let Some(module) = cursor {
            left_ancestors.insert(module);
            cursor = tcx
                .opt_local_parent(module)
                .filter(|parent| tcx.def_kind(*parent) == DefKind::Mod);
        }

        // Walk the right ancestry until it enters the left module chain.
        let mut cursor = Some(right);
        while let Some(module) = cursor {
            if left_ancestors.contains(&module) {
                return module;
            }
            cursor = tcx
                .opt_local_parent(module)
                .filter(|parent| tcx.def_kind(*parent) == DefKind::Mod);
        }
        CRATE_DEF_ID
    }

    /// Returns the canonical source spelling used by diagnostics and suggestions.
    pub(crate) const fn source(self) -> &'static str {
        match self {
            Self::Private => "private",
            Self::Super => "pub(super)",
            Self::Crate => "pub(crate)",
            Self::Public => "pub",
        }
    }
}

impl fmt::Display for VisibilityBoundary {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.source())
    }
}

// -----------------------------------------------------------------------------
// Tests: Unit tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::VisibilityBoundary;

    #[test]
    fn renders_the_canonical_visibility_vocabulary() {
        assert_eq!(VisibilityBoundary::Private.source(), "private");
        assert_eq!(VisibilityBoundary::Super.source(), "pub(super)");
        assert_eq!(VisibilityBoundary::Crate.source(), "pub(crate)");
        assert_eq!(VisibilityBoundary::Public.source(), "pub");
    }
}
