#[cfg(feature = "strum")]
extern crate rustc_errors;
extern crate rustc_hir;
#[cfg(feature = "strum")]
extern crate rustc_span;

#[cfg(feature = "strum")]
use std::borrow::Cow;

#[cfg(feature = "strum")]
use rustc_errors::DiagDecorator;
use rustc_hir::{ImplItem, Item};
#[cfg(feature = "strum")]
use rustc_lint::LintContext;
use rustc_lint::{LateContext, LateLintPass};
#[cfg(feature = "strum")]
use rustc_span::Span;

#[cfg(feature = "strum")]
use super::config::DeriveResolutionConfig;
#[cfg(feature = "strum")]
use crate::rules::strum::utils::enumeration::CollectionCandidate;
#[cfg(feature = "strum")]
use crate::utils::config::LibraryConfig;
#[cfg(feature = "strum")]
use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Missing provider resolution
// -----------------------------------------------------------------------------

/// Exhaustive enum collection whose two compatible Strum APIs need explicit policy.
#[cfg(feature = "strum")]
struct Violation {
    span: Span,
}

#[cfg(feature = "strum")]
impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("enum variant collection has multiple eligible framework resolutions")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "`EnumIter` exposes an iterator protocol while `VariantArray` exposes a shared static slice; dependency presence cannot choose that API policy",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "set `derive_resolution.enum_variant_collection` to `strum_enum_iter` or `strum_variant_array` in the `rlib-lint` Dylint table",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            FRAMEWORK_RESOLUTION_REQUIRED,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// FrameworkResolutionRequired: Provider arbitration
// -----------------------------------------------------------------------------

/// Reports unresolved framework choices without selecting a provider.
struct FrameworkResolutionRequired {
    #[cfg(feature = "strum")]
    config: DeriveResolutionConfig,
}

impl FrameworkResolutionRequired {
    fn new() -> Self {
        Self {
            #[cfg(feature = "strum")]
            config: LibraryConfig::load().derive_resolution,
        }
    }

    #[cfg(feature = "strum")]
    fn check_candidate(&self, cx: &LateContext<'_>, candidate: Option<CollectionCandidate>) {
        let Some(candidate) = candidate else {
            return;
        };
        if candidate.providers().len() > 1 && self.config.enum_variant_collection().is_none() {
            Violation {
                span: candidate.span,
            }
            .emit(cx);
        }
    }
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub FRAMEWORK_RESOLUTION_REQUIRED,
    Warn,
    "requires explicit configuration to choose between framework remediations",
    FrameworkResolutionRequired::new()
}

impl LateLintPass<'_> for FrameworkResolutionRequired {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        #[cfg(feature = "strum")]
        self.check_candidate(cx, CollectionCandidate::from_item(cx, item));
        #[cfg(not(feature = "strum"))]
        let _ = (cx, item);
    }

    fn check_impl_item(&mut self, cx: &LateContext<'_>, item: &ImplItem<'_>) {
        #[cfg(feature = "strum")]
        self.check_candidate(cx, CollectionCandidate::from_impl_item(cx, item));
        #[cfg(not(feature = "strum"))]
        let _ = (cx, item);
    }
}

#[cfg(test)]
mod tests {
    use super::super::config::DeriveResolutionConfig;

    #[test]
    fn accepts_an_absent_provider_choice() {
        let config = toml::from_str::<DeriveResolutionConfig>("")
            .expect("empty resolution config should parse");
        #[cfg(feature = "strum")]
        assert!(config.enum_variant_collection().is_none());
    }
}
