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

/// Carries the `Violation` state used by this analysis.
struct Violation {
    /// Stores the `owner` value used by this analysis.
    owner: rustc_hir::HirId,
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `enum_name` value used by this analysis.
    enum_name: Symbol,
    /// Stores the `is_public` value used by this analysis.
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

/// Carries the `StrumManualEnumStringParsers` state used by this analysis.
struct StrumManualEnumStringParsers {
    /// Stores the `provider` value used by this analysis.
    provider: Option<StringParserProvider>,
    /// Stores the `catalog` value used by this analysis.
    catalog: ContractCatalog,
    /// Stores the `candidates` value used by this analysis.
    candidates: Vec<StringParserCandidate>,
}

impl StrumManualEnumStringParsers {
    /// Performs the `new` operation for this value.
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

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub STRUM_MANUAL_ENUM_STRING_PARSERS,
    Warn,
    "finds manual enum string parsers reproducible by Strum",
    StrumManualEnumStringParsers::new()
}

impl LateLintPass<'_> for StrumManualEnumStringParsers {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
    }

    fn check_impl_item(&mut self, cx: &LateContext<'_>, item: &ImplItem<'_>) {
        let Some(analyze_candidate) = StringParserCandidate::from_impl_item(cx, item) else {
            return;
        };
        self.candidates.push(analyze_candidate);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        if StringParserProvider::selected(cx, self.provider)
            != Some(StringParserProvider::StrumEnumString)
        {
            return;
        }
        let contracts = self.catalog.contracts();
        for analyze_candidate in self.candidates.drain(..) {
            // Prepare the values used by this stage.
            let Some(contract) = contracts.iter().find(|contract| {
                contract.def_id == analyze_candidate.enum_def
                    && contract.variants.iter().all(|variant| {
                        analyze_candidate.names.get(&variant.def_id) == Some(&variant.parser_names)
                    })
            }) else {
                continue;
            };

            // Perform the next step of the analysis.
            Violation {
                owner: analyze_candidate.owner,
                span: analyze_candidate.span,
                enum_name: contract.name,
                is_public: analyze_candidate.is_public,
            }
            .emit(cx);
        }
    }
}
