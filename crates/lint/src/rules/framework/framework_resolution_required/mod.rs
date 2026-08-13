#[cfg(any(feature = "strum", all(feature = "thiserror", feature = "derive_more")))]
extern crate rustc_errors;
extern crate rustc_hir;
#[cfg(any(feature = "strum", all(feature = "thiserror", feature = "derive_more")))]
extern crate rustc_span;

#[cfg(any(feature = "strum", all(feature = "thiserror", feature = "derive_more")))]
use std::borrow::Cow;

#[cfg(any(feature = "strum", all(feature = "thiserror", feature = "derive_more")))]
use rustc_errors::DiagDecorator;
use rustc_hir::{ImplItem, Item};
#[cfg(any(feature = "strum", all(feature = "thiserror", feature = "derive_more")))]
use rustc_lint::LintContext;
use rustc_lint::{LateContext, LateLintPass};
#[cfg(any(feature = "strum", all(feature = "thiserror", feature = "derive_more")))]
use rustc_span::Span;

#[cfg(any(feature = "strum", all(feature = "thiserror", feature = "derive_more")))]
use super::config::DeriveResolutionConfig;
#[cfg(feature = "strum")]
use crate::rules::strum::utils::authored_contracts::{
    DisplayCandidate, DisplayProvider, StringParserCandidate, StringParserProvider,
};
#[cfg(feature = "strum")]
use crate::rules::strum::utils::contracts::ContractCatalog;
#[cfg(feature = "strum")]
use crate::rules::strum::utils::enumeration::CollectionCandidate;
#[cfg(all(feature = "thiserror", feature = "derive_more"))]
use crate::rules::thiserror::contracts::ThiserrorContractCatalog;
#[cfg(all(feature = "thiserror", feature = "derive_more"))]
use crate::rules::thiserror::manual_error::Catalog as ManualErrorCatalog;
#[cfg(all(feature = "thiserror", feature = "derive_more"))]
use crate::rules::thiserror::manual_from::Candidate as ManualFromCandidate;
#[cfg(any(feature = "strum", all(feature = "thiserror", feature = "derive_more")))]
use crate::utils::config::LibraryConfig;
#[cfg(any(feature = "strum", all(feature = "thiserror", feature = "derive_more")))]
use crate::utils::diagnostic::LateViolation;
#[cfg(feature = "strum")]
use crate::utils::variant_methods::{PredicateFamily, PredicateFamilyAnalyzer};

/// Exhaustive enum collection whose two compatible Strum APIs need explicit policy.
#[cfg(feature = "strum")]
struct CollectionViolation {
    /// Stores the `span` value used by this analysis.
    span: Span,
}

#[cfg(feature = "strum")]
impl LateViolation for CollectionViolation {
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

/// Complete enum predicate family whose two derive providers need explicit policy.
#[cfg(feature = "strum")]
struct PredicateViolation {
    /// Stores the `span` value used by this analysis.
    span: Span,
}

#[cfg(feature = "strum")]
impl LateViolation for PredicateViolation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("enum variant predicates have multiple eligible framework resolutions")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "Strum `EnumIs` and derive_more `IsVariant` generate the same predicate family; dependency presence cannot choose that provider policy",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "set `derive_resolution.enum_variant_predicates` to `strum_enum_is` or `derive_more_is_variant` in the `rlib-lint` Dylint table",
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

#[cfg(feature = "strum")]
/// Carries the `DisplayViolation` state used by this analysis.
struct DisplayViolation {
    /// Stores the `span` value used by this analysis.
    span: Span,
}

#[cfg(feature = "strum")]
impl LateViolation for DisplayViolation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("enum display has multiple eligible framework resolutions")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("Strum and derive_more can both generate this static enum `Display` contract")
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "set `derive_resolution.enum_display` to `strum_display` or `derive_more_display`",
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

#[cfg(feature = "strum")]
/// Carries the `ParserViolation` state used by this analysis.
struct ParserViolation {
    /// Stores the `span` value used by this analysis.
    span: Span,
}

#[cfg(feature = "strum")]
impl LateViolation for ParserViolation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("enum string parsing has multiple eligible framework resolutions")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "Strum `EnumString` and derive_more `FromStr` can both generate this flat unit-enum parser",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "set `derive_resolution.enum_string_parsing` to `strum_enum_string` or `derive_more_from_str`",
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

#[cfg(all(feature = "thiserror", feature = "derive_more"))]
/// Carries the `ErrorConversionViolation` state used by this analysis.
struct ErrorConversionViolation {
    /// Stores the `span` value used by this analysis.
    span: Span,
}

#[cfg(all(feature = "thiserror", feature = "derive_more"))]
impl LateViolation for ErrorConversionViolation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("error-variant conversion has multiple eligible derive providers")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "thiserror `#[from]` and derive_more `From` can both generate this exact source-bearing variant conversion",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "set `derive_resolution.error_variant_conversion` to `thiserror_from` or `derive_more_from`",
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

#[cfg(all(feature = "thiserror", feature = "derive_more"))]
/// Carries the `ErrorImplementationViolation` state used by this analysis.
struct ErrorImplementationViolation {
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `name` value used by this analysis.
    name: String,
}

#[cfg(all(feature = "thiserror", feature = "derive_more"))]
impl LateViolation for ErrorImplementationViolation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "error contract for `{}` has multiple eligible derive providers",
            self.name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "thiserror `Error` and derive_more `Display` plus `Error` can both generate this static message and conventional source chain",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "set `derive_resolution.error_implementation` to `thiserror_error` or `derive_more_error`",
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

/// Reports unresolved framework choices without selecting a provider.
struct FrameworkResolutionRequired {
    #[cfg(any(feature = "strum", all(feature = "thiserror", feature = "derive_more")))]
    /// Stores the `config` value used by this analysis.
    config: DeriveResolutionConfig,
    #[cfg(feature = "strum")]
    /// Stores the `predicates` value used by this analysis.
    predicates: PredicateFamilyAnalyzer,
    #[cfg(feature = "strum")]
    /// Stores the `catalog` value used by this analysis.
    catalog: ContractCatalog,
    #[cfg(feature = "strum")]
    /// Stores the `displays` value used by this analysis.
    displays: Vec<DisplayCandidate>,
    #[cfg(feature = "strum")]
    /// Stores the `parsers` value used by this analysis.
    parsers: Vec<StringParserCandidate>,
    #[cfg(all(feature = "thiserror", feature = "derive_more"))]
    /// Stores the `thiserror_catalog` value used by this analysis.
    thiserror_catalog: ThiserrorContractCatalog,
    #[cfg(all(feature = "thiserror", feature = "derive_more"))]
    /// Stores the `error_conversions` value used by this analysis.
    error_conversions: Vec<ManualFromCandidate>,
    #[cfg(all(feature = "thiserror", feature = "derive_more"))]
    /// Stores the `manual_errors` value used by this analysis.
    manual_errors: ManualErrorCatalog,
}

impl FrameworkResolutionRequired {
    /// Performs the `new` operation for this value.
    fn new() -> Self {
        Self {
            #[cfg(any(feature = "strum", all(feature = "thiserror", feature = "derive_more")))]
            config: LibraryConfig::load().derive_resolution,
            #[cfg(feature = "strum")]
            predicates: PredicateFamilyAnalyzer::default(),
            #[cfg(feature = "strum")]
            catalog: ContractCatalog::default(),
            #[cfg(feature = "strum")]
            displays: Vec::new(),
            #[cfg(feature = "strum")]
            parsers: Vec::new(),
            #[cfg(all(feature = "thiserror", feature = "derive_more"))]
            thiserror_catalog: ThiserrorContractCatalog::default(),
            #[cfg(all(feature = "thiserror", feature = "derive_more"))]
            error_conversions: Vec::new(),
            #[cfg(all(feature = "thiserror", feature = "derive_more"))]
            manual_errors: ManualErrorCatalog::default(),
        }
    }

    #[cfg(feature = "strum")]
    /// Performs the `check_candidate` operation for this value.
    fn check_candidate(
        &self,
        cx: &LateContext<'_>,
        analyze_candidate: Option<CollectionCandidate>,
    ) {
        // Prepare the values used by this stage.
        let Some(analyze_candidate) = analyze_candidate else {
            return;
        };
        if !(analyze_candidate.providers().len() > 1
            && self.config.enum_variant_collection().is_none())
        // Perform the next step of the analysis.
        {
            return;
        }
        CollectionViolation {
            span: analyze_candidate.span,
        }
        .emit(cx);
    }

    #[cfg(feature = "strum")]
    /// Performs the `emit_unresolved_predicates` operation for this value.
    fn emit_unresolved_predicates(&self, cx: &LateContext<'_>) {
        for family in self.predicates.complete_families(cx) {
            if !(PredicateFamily::providers(cx).len() > 1
                && self.config.enum_variant_predicates().is_none())
            {
                continue;
            }
            PredicateViolation { span: family.span }.emit(cx);
        }
    }

    #[cfg(feature = "strum")]
    /// Performs the `emit_unresolved_text_contracts` operation for this value.
    fn emit_unresolved_text_contracts(&self, cx: &LateContext<'_>) {
        let contracts = self.catalog.contracts();
        if DisplayProvider::providers(cx).len() > 1 && self.config.enum_display().is_none() {
            for display in &self.displays {
                if !(contracts.iter().any(|contract| {
                    contract.def_id == display.enum_def
                        && contract.variants.iter().all(|variant| {
                            display.values.get(&variant.def_id) == Some(&variant.preferred_name)
                        })
                })) {
                    continue;
                }
                DisplayViolation { span: display.span }.emit(cx);
            }
        }
        if StringParserProvider::providers(cx).len() <= 1
            || self.config.enum_string_parsing().is_some()
        {
            return;
        }
        for parser in &self.parsers {
            if !(contracts.iter().any(|contract| {
                contract.def_id == parser.enum_def
                    && contract.variants.iter().all(|variant| {
                        parser.names.get(&variant.def_id) == Some(&variant.parser_names)
                    })
            })) {
                continue;
            }
            ParserViolation { span: parser.span }.emit(cx);
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
        {
            self.check_candidate(cx, CollectionCandidate::from_item(cx, item));
            self.catalog.check_item(cx, item);
        }
        #[cfg(all(feature = "thiserror", feature = "derive_more"))]
        {
            self.thiserror_catalog.check_item(cx, item);
            self.manual_errors.check_item(cx, item);
        }
        #[cfg(not(any(feature = "strum", all(feature = "thiserror", feature = "derive_more"))))]
        let _ = (cx, item);
    }

    fn check_impl_item(&mut self, cx: &LateContext<'_>, item: &ImplItem<'_>) {
        #[cfg(feature = "strum")]
        {
            self.check_candidate(cx, CollectionCandidate::from_impl_item(cx, item));
            self.predicates.check_impl_item(cx, item);
            if let Some(display) = DisplayCandidate::from_impl_item(cx, item) {
                self.displays.push(display);
            }
            if let Some(parser) = StringParserCandidate::from_impl_item(cx, item) {
                self.parsers.push(parser);
            }
        }
        #[cfg(all(feature = "thiserror", feature = "derive_more"))]
        let Some(analyze_candidate) = ManualFromCandidate::from_impl_item(cx, item) else {
            return;
        };
        self.error_conversions.push(analyze_candidate);

        #[cfg(not(any(feature = "strum", all(feature = "thiserror", feature = "derive_more"))))]
        let _ = (cx, item);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        let _ = cx;
        #[cfg(feature = "strum")]
        {
            self.emit_unresolved_predicates(cx);
            self.emit_unresolved_text_contracts(cx);
        }
        #[cfg(all(feature = "thiserror", feature = "derive_more"))]
        if self.config.error_variant_conversion().is_none() {
            // Reject inputs that do not satisfy this stage.
            for analyze_candidate in &self.error_conversions {
                if self
                    .thiserror_catalog
                    .derived_type(analyze_candidate.definition)
                    .is_none()
                {
                    continue;
                }
                ErrorConversionViolation {
                    span: analyze_candidate.span,
                }
                .emit(cx);
            }
        }
        #[cfg(all(feature = "thiserror", feature = "derive_more"))]
        if self.config.error_implementation().is_some() {
            return;
        }
        for analyze_candidate in self.manual_errors.candidates() {
            // Reject inputs that do not satisfy this stage.
            if analyze_candidate
                .source_field
                .as_deref()
                .is_some_and(|field| field != "source")
            {
                continue;
            }

            // Perform the next step of the analysis.
            ErrorImplementationViolation {
                span: analyze_candidate.span,
                name: analyze_candidate.name,
            }
            .emit(cx);
        }

        #[cfg(not(any(feature = "strum", all(feature = "thiserror", feature = "derive_more"))))]
        let _ = cx;
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
        {
            assert!(config.enum_variant_collection().is_none());
            assert!(config.enum_variant_predicates().is_none());
            assert!(config.enum_display().is_none());
            assert!(config.enum_string_parsing().is_none());
        }
    }
}
