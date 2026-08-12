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

struct Violation {
    owner: rustc_hir::HirId,
    span: Span,
    enum_name: Symbol,
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

struct StrumManualEnumStringParsers {
    provider: Option<StringParserProvider>,
    catalog: ContractCatalog,
    candidates: Vec<StringParserCandidate>,
}

impl StrumManualEnumStringParsers {
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
        if let Some(candidate) = StringParserCandidate::from_impl_item(cx, item) {
            self.candidates.push(candidate);
        }
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
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
                        candidate.names.get(&variant.def_id) == Some(&variant.parser_names)
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
