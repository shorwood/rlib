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
use crate::rules::thiserror::manual_error::ManualErrorCatalog;
#[cfg(all(feature = "thiserror", feature = "derive_more"))]
use crate::rules::thiserror::manual_from::Candidate as ManualFromCandidate;
#[cfg(any(feature = "strum", all(feature = "thiserror", feature = "derive_more")))]
use crate::utils::config::LibraryConfig;
#[cfg(any(feature = "strum", all(feature = "thiserror", feature = "derive_more")))]
use crate::utils::diagnostic::LateViolation;
#[cfg(feature = "strum")]
use crate::utils::variant_methods::{PredicateFamily, PredicateFamilyAnalyzer};

// -----------------------------------------------------------------------------
// Violation: Unresolved framework provider choice
// -----------------------------------------------------------------------------

/// Derivable contract with multiple eligible providers and no configured owner.
#[cfg(any(feature = "strum", all(feature = "thiserror", feature = "derive_more")))]
enum Violation {
    /// Exhaustive enum collection supported by two different Strum APIs.
    #[cfg(feature = "strum")]
    Collection {
        /// Exhaustive collection contract needing an explicit API choice.
        span: Span,
    },
    /// Complete enum predicate family supported by Strum and `derive_more`.
    #[cfg(feature = "strum")]
    Predicate {
        /// Predicate family supported by both enabled derive libraries.
        span: Span,
    },
    /// Static enum display contract supported by Strum and `derive_more`.
    #[cfg(feature = "strum")]
    Display {
        /// Static display implementation supported by both enabled libraries.
        span: Span,
    },
    /// Flat unit-enum parser supported by Strum and `derive_more`.
    #[cfg(feature = "strum")]
    Parser {
        /// Unit-enum parser supported by both enabled libraries.
        span: Span,
    },
    /// Exact error-variant conversion supported by thiserror and `derive_more`.
    #[cfg(all(feature = "thiserror", feature = "derive_more"))]
    ErrorConversion {
        /// Source-bearing variant conversion needing an explicit provider.
        span: Span,
    },
    /// Complete error contract supported by thiserror and `derive_more`.
    #[cfg(all(feature = "thiserror", feature = "derive_more"))]
    ErrorImplementation {
        /// Complete manual error contract needing an explicit provider.
        span: Span,
        /// Error type named in the diagnostic.
        name: String,
    },
}

#[cfg(any(feature = "strum", all(feature = "thiserror", feature = "derive_more")))]
impl Violation {
    /// Returns the authored contract receiving the provider-choice diagnostic.
    const fn span(&self) -> Span {
        match self {
            #[cfg(feature = "strum")]
            Self::Collection { span }
            | Self::Predicate { span }
            | Self::Display { span }
            | Self::Parser { span } => *span,
            #[cfg(all(feature = "thiserror", feature = "derive_more"))]
            Self::ErrorConversion { span } | Self::ErrorImplementation { span, .. } => *span,
        }
    }
}

#[cfg(any(feature = "strum", all(feature = "thiserror", feature = "derive_more")))]
impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        match self {
            #[cfg(feature = "strum")]
            Self::Collection { .. } => {
                Cow::Borrowed("enum variant collection has multiple eligible framework resolutions")
            }
            #[cfg(feature = "strum")]
            Self::Predicate { .. } => Cow::Borrowed(
                "enum variant predicates have multiple eligible framework resolutions",
            ),
            #[cfg(feature = "strum")]
            Self::Display { .. } => {
                Cow::Borrowed("enum display has multiple eligible framework resolutions")
            }
            #[cfg(feature = "strum")]
            Self::Parser { .. } => {
                Cow::Borrowed("enum string parsing has multiple eligible framework resolutions")
            }
            #[cfg(all(feature = "thiserror", feature = "derive_more"))]
            Self::ErrorConversion { .. } => {
                Cow::Borrowed("error-variant conversion has multiple eligible derive providers")
            }
            #[cfg(all(feature = "thiserror", feature = "derive_more"))]
            Self::ErrorImplementation { name, .. } => Cow::Owned(format!(
                "error contract for `{name}` has multiple eligible derive providers"
            )),
        }
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        match self {
            #[cfg(feature = "strum")]
            Self::Collection { .. } => Cow::Borrowed(
                "`EnumIter` exposes an iterator while `VariantArray` exposes a shared slice; dependency presence cannot choose that API contract",
            ),
            #[cfg(feature = "strum")]
            Self::Predicate { .. } => Cow::Borrowed(
                "Strum `EnumIs` and derive_more `IsVariant` generate the same predicates; dependency presence cannot choose their owner",
            ),
            #[cfg(feature = "strum")]
            Self::Display { .. } => Cow::Borrowed(
                "Strum and derive_more can both generate this static enum `Display` contract",
            ),
            #[cfg(feature = "strum")]
            Self::Parser { .. } => Cow::Borrowed(
                "Strum `EnumString` and derive_more `FromStr` can both generate this unit-enum parser",
            ),
            #[cfg(all(feature = "thiserror", feature = "derive_more"))]
            Self::ErrorConversion { .. } => Cow::Borrowed(
                "thiserror `#[from]` and derive_more `From` can both generate this source-bearing variant conversion",
            ),
            #[cfg(all(feature = "thiserror", feature = "derive_more"))]
            Self::ErrorImplementation { .. } => Cow::Borrowed(
                "thiserror and derive_more can both generate this static message and conventional source chain",
            ),
        }
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        match self {
            #[cfg(feature = "strum")]
            Self::Collection { .. } => Cow::Borrowed(
                "set `derive_resolution.enum_variant_collection` to `strum_enum_iter` or `strum_variant_array`",
            ),
            #[cfg(feature = "strum")]
            Self::Predicate { .. } => Cow::Borrowed(
                "set `derive_resolution.enum_variant_predicates` to `strum_enum_is` or `derive_more_is_variant`",
            ),
            #[cfg(feature = "strum")]
            Self::Display { .. } => Cow::Borrowed(
                "set `derive_resolution.enum_display` to `strum_display` or `derive_more_display`",
            ),
            #[cfg(feature = "strum")]
            Self::Parser { .. } => Cow::Borrowed(
                "set `derive_resolution.enum_string_parsing` to `strum_enum_string` or `derive_more_from_str`",
            ),
            #[cfg(all(feature = "thiserror", feature = "derive_more"))]
            Self::ErrorConversion { .. } => Cow::Borrowed(
                "set `derive_resolution.error_variant_conversion` to `thiserror_from` or `derive_more_from`",
            ),
            #[cfg(all(feature = "thiserror", feature = "derive_more"))]
            Self::ErrorImplementation { .. } => Cow::Borrowed(
                "set `derive_resolution.error_implementation` to `thiserror_error` or `derive_more_error`",
            ),
        }
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            FRAMEWORK_RESOLUTION_REQUIRED,
            self.span(),
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// FrameworkResolutionRequired: Explicit provider-choice policy
// -----------------------------------------------------------------------------

/// Reports unresolved framework choices without selecting a provider.
struct FrameworkResolutionRequired {
    /// Explicit provider choices for overlapping derive contracts.
    #[cfg(any(feature = "strum", all(feature = "thiserror", feature = "derive_more")))]
    config: DeriveResolutionConfig,
    /// Authored predicate families eligible for more than one provider.
    #[cfg(feature = "strum")]
    predicates: PredicateFamilyAnalyzer,
    /// Authored enum naming contracts used to validate text behavior.
    #[cfg(feature = "strum")]
    catalog: ContractCatalog,
    /// Manual display contracts awaiting provider resolution.
    #[cfg(feature = "strum")]
    displays: Vec<DisplayCandidate>,
    /// Manual string parsers awaiting provider resolution.
    #[cfg(feature = "strum")]
    parsers: Vec<StringParserCandidate>,
    /// Local thiserror contracts used to confirm error targets.
    #[cfg(all(feature = "thiserror", feature = "derive_more"))]
    thiserror_catalog: ThiserrorContractCatalog,
    /// Exact manual error conversions eligible for competing derives.
    #[cfg(all(feature = "thiserror", feature = "derive_more"))]
    error_conversions: Vec<ManualFromCandidate>,
    /// Complete manual error contracts eligible for competing derives.
    #[cfg(all(feature = "thiserror", feature = "derive_more"))]
    manual_errors: ManualErrorCatalog,
}

impl FrameworkResolutionRequired {
    /// Loads explicit resolution and initializes provider-specific analysis.
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

    /// Reports an enum collection contract when multiple APIs remain unselected.
    #[cfg(feature = "strum")]
    fn check_candidate(&self, cx: &LateContext<'_>, candidate: Option<CollectionCandidate>) {
        let Some(candidate) = candidate else {
            return;
        };
        if !(candidate.providers().len() > 1 && self.config.enum_variant_collection().is_none()) {
            return;
        }
        Violation::Collection {
            span: candidate.span,
        }
        .emit(cx);
    }

    /// Reports complete predicate families with no configured provider.
    #[cfg(feature = "strum")]
    fn emit_unresolved_predicates(&self, cx: &LateContext<'_>) {
        for family in self.predicates.complete_families(cx) {
            if !(PredicateFamily::providers(cx).len() > 1
                && self.config.enum_variant_predicates().is_none())
            {
                continue;
            }
            Violation::Predicate { span: family.span }.emit(cx);
        }
    }

    /// Reports display and parser contracts with no configured provider.
    #[cfg(feature = "strum")]
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
                Violation::Display { span: display.span }.emit(cx);
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
            Violation::Parser { span: parser.span }.emit(cx);
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
        {
            let Some(candidate) = ManualFromCandidate::from_impl_item(cx, item) else {
                return;
            };
            self.error_conversions.push(candidate);
        }

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
            for candidate in &self.error_conversions {
                if self
                    .thiserror_catalog
                    .derived_type(candidate.definition)
                    .is_none()
                {
                    continue;
                }
                Violation::ErrorConversion {
                    span: candidate.span,
                }
                .emit(cx);
            }
        }
        #[cfg(all(feature = "thiserror", feature = "derive_more"))]
        {
            if self.config.error_implementation().is_some() {
                return;
            }
            for candidate in self.manual_errors.candidates() {
                if candidate
                    .source_field
                    .as_deref()
                    .is_some_and(|field| field != "source")
                {
                    continue;
                }

                Violation::ErrorImplementation {
                    span: candidate.span,
                    name: candidate.name,
                }
                .emit(cx);
            }
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
