extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{ImplItem, Item};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::{Span, Symbol};

use super::utils::authored_contracts::{StringParserCandidate, StringParserProvider};
use super::utils::contracts::ContractCatalog;
use crate::utils::config::LibraryConfig;
use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Hand-maintained enum string parser
// -----------------------------------------------------------------------------

/// Complete string-to-variant mapping reproducible by the configured provider.
struct Violation {
    /// Declaration whose lint level governs this finding.
    owner: rustc_hir::HirId,
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Enum name quoted in the diagnostic.
    enum_name: Symbol,
    /// Whether the declaration is visible outside its defining module.
    is_public: bool,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "`{}` string parser is maintained manually",
            self.enum_name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "the exhaustive string match duplicates unit-variant names and failure handling",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "derive `strum::EnumString` and remove the equivalent `FromStr` implementation",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            STRUM_MANUAL_ENUM_STRING_PARSERS,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.note(self.rationale_message().into_owned());
                if self.is_public {
                    diag.note("this trait implementation is public API; compare the generated error type before migration");
                }
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// StrumManualEnumStringParsers: Declarative parser policy
// -----------------------------------------------------------------------------

/// Finds complete manual parsers after provider resolution.
struct StrumManualEnumStringParsers {
    /// Explicitly resolved framework provider, when one is available.
    provider: Option<StringParserProvider>,
    /// Effective Strum contracts consulted after generated items are associated.
    catalog: ContractCatalog,
    /// Authored enum uses awaiting association with completed Strum contracts.
    candidates: Vec<StringParserCandidate>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub STRUM_MANUAL_ENUM_STRING_PARSERS,
    Warn,
    "finds manual enum string parsers reproducible by Strum",
    StrumManualEnumStringParsers::new()
}

impl StrumManualEnumStringParsers {
    /// Starts parser analysis with no authored parser candidates.
    fn new() -> Self {
        Self {
            provider: LibraryConfig::load()
                .derive_resolution
                .enum_string_parsing(),
            catalog: ContractCatalog::default(),
            candidates: Vec::new(),
        }
    }
}
impl LateLintPass<'_> for StrumManualEnumStringParsers {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
    }

    fn check_impl_item(&mut self, cx: &LateContext<'_>, item: &ImplItem<'_>) {
        // Implementation items without a complete parser are unrelated.
        let Some(candidate) = StringParserCandidate::from_impl_item(cx, item) else {
            return;
        };
        self.candidates.push(candidate);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        // Replacement is valid only when Strum owns enum-string parsing.
        if StringParserProvider::selected(cx, self.provider)
            != Some(StringParserProvider::StrumEnumString)
        {
            return;
        }
        let contracts = self.catalog.contracts();
        for candidate in self.candidates.drain(..) {
            let Some(contract) = contracts.iter().find(|contract| {
                contract.def_id == candidate.enum_def
                    && contract.variants.iter().all(|variant| {
                        !variant.is_disabled
                            && !variant.is_ascii_case_insensitive
                            && !variant.is_default_capture
                    })
                    && contract.variants.iter().all(|variant| {
                        candidate.names.get(&variant.def_id).is_some_and(|names| {
                            let mut names = names.clone();
                            let mut parser_names = variant.parser_names.clone();
                            names.sort();
                            parser_names.sort();
                            names == parser_names
                        })
                    })
            }) else {
                continue;
            };

            Violation {
                owner: candidate.owner,
                span: candidate.span,
                enum_name: contract.name,
                is_public: candidate.is_public,
            }
            .emit(cx);
        }
    }
}
