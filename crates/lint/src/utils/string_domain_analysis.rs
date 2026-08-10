extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_span;

use std::collections::{HashMap, HashSet};

use rustc_hir::def::DefKind;
use rustc_hir::{Body, FieldDef, Item, ItemKind};
use rustc_lint::{LateContext, LintContext};
use rustc_span::def_id::LocalDefId;
use rustc_span::{Span, Symbol};

use super::identifier_case;
use super::parameter_analysis::{Parameter, ParameterSignature};
use super::parameter_kind::{ParameterKind, ParameterTypeExt};
use super::string_domain_revalidation::StringDomainBodyExt;
use super::string_domain_vocabulary::StringDomainSymbolExt;

/// Smallest consumer family that proves repeated domain revalidation.
const MIN_REVALIDATION_CONSUMER_COUNT: usize = 2;

// -----------------------------------------------------------------------------
// DomainFinding: String domain diagnostics
// -----------------------------------------------------------------------------

/// Supporting declaration attached to a domain-boundary diagnostic.
pub struct DomainFindingLabel {
    /// Declaration source range.
    pub(crate) span: Span,
    /// Explanation of the declaration's contribution to the finding.
    pub(crate) message: String,
}

/// Free functions and raw fields collectively behaving like a missing domain type.
pub struct DomainFindingFamily {
    /// Primary source range for the family-level warning.
    pub(crate) span: Span,
    /// Inferred `PascalCase` domain concept.
    pub(crate) domain: String,
    /// Existing same-module type already naming the inferred concept.
    pub(crate) existing_type: Option<String>,
    /// Supporting declarations in source order.
    pub(crate) labels: Vec<DomainFindingLabel>,
}

/// Raw textual parameters that repeatedly re-establish one domain invariant.
pub struct DomainFindingRevalidation {
    /// Primary parameter source range.
    pub(crate) span: Span,
    /// Inferred `PascalCase` domain concept.
    pub(crate) domain: String,
    /// Consumer parameters that repeat invariant establishment.
    pub(crate) labels: Vec<DomainFindingLabel>,
}

// -----------------------------------------------------------------------------
// DomainEvidence: Module local string domain evidence
// -----------------------------------------------------------------------------

/// Stable key for evidence belonging to one module-local domain.
#[derive(Clone, Eq, Hash, PartialEq)]
struct DomainEvidenceKey {
    /// Module containing the evidence.
    module: LocalDefId,
    /// Inferred `PascalCase` domain concept.
    domain: String,
}

/// Free function contributing behavior to an inferred string domain.
#[derive(Clone)]
struct DomainEvidenceFunction {
    /// Function definition identity.
    def_id: LocalDefId,
    /// Containing source module.
    module: LocalDefId,
    /// Function name span.
    span: Span,
    /// Authored function name.
    name: Symbol,
    /// Inferred domain concept.
    domain: String,
}

/// Raw textual field reinforcing an inferred string domain.
#[derive(Clone)]
struct DomainEvidenceField {
    /// Containing source module.
    module: LocalDefId,
    /// Field name span.
    span: Span,
    /// Authored field name.
    name: Symbol,
    /// Inferred domain concept.
    domain: String,
}

/// Existing type whose name already expresses a candidate domain concept.
struct DomainEvidenceType {
    /// Containing source module.
    module: LocalDefId,
    /// Type name span.
    span: Span,
    /// Authored type name.
    name: Symbol,
}

// -----------------------------------------------------------------------------
// DomainEvidenceRevalidation: Raw string invariant checks
// -----------------------------------------------------------------------------

/// Identity and module placement of one raw-string consumer.
#[derive(Clone)]
struct DomainEvidenceRevalidationConsumer {
    /// Consumer definition identity.
    def_id: LocalDefId,
    /// Containing source module.
    module: LocalDefId,
    /// Authored consumer name.
    name: Symbol,
}

/// Consumer that establishes an invariant on a raw string parameter.
#[derive(Clone)]
struct DomainEvidenceRevalidation {
    /// Identity and placement of the consuming function.
    consumer: DomainEvidenceRevalidationConsumer,
    /// Parameter name span.
    span: Span,
    /// Authored parameter name.
    parameter: Symbol,
    /// Inferred domain concept.
    domain: String,
}

impl DomainEvidenceRevalidation {
    /// Builds one revalidation record when a parameter exposes a precise domain.
    fn from_parameter(
        signature: &ParameterSignature,
        module: LocalDefId,
        parameter: &Parameter,
    ) -> Option<Self> {
        // Resolve the domain and preserve the consuming function as one semantic identity.
        let domain = parameter.name.parameter_domain()?;
        let consumer = DomainEvidenceRevalidationConsumer {
            def_id: signature.def_id,
            module,
            name: signature.name,
        };

        // Keep the parameter site and vocabulary beside its consuming function identity.
        Some(Self {
            consumer,
            span: parameter.span,
            parameter: parameter.name,
            domain,
        })
    }
}

// -----------------------------------------------------------------------------
// DomainAnalyzer: Evidence aggregation and diagnostic precedence
// -----------------------------------------------------------------------------

/// Complete crate-local evidence used by the string-domain lints.
#[derive(Default)]
pub struct DomainAnalyzer {
    /// Free-function behavior grouped later by module and domain.
    functions: Vec<DomainEvidenceFunction>,
    /// Raw textual fields grouped later by module and domain.
    fields: Vec<DomainEvidenceField>,
    /// Existing module-local types used to avoid proposing duplicates.
    types: Vec<DomainEvidenceType>,
    /// Consumer-side invariant establishment.
    revalidations: Vec<DomainEvidenceRevalidation>,
}

impl DomainAnalyzer {
    /// Selects textual parameters from one analyzed signature.
    fn textual_parameters(signature: &ParameterSignature) -> Vec<&Parameter> {
        signature
            .parameters
            .iter()
            .filter(|parameter| parameter.interchangeable == Some(ParameterKind::Text))
            .collect()
    }

    /// Builds source-ordered labels supporting one function-family finding.
    fn family_labels(
        domain: &str,
        functions: &[&DomainEvidenceFunction],
        fields: Vec<&DomainEvidenceField>,
        existing_type: Option<&DomainEvidenceType>,
    ) -> Vec<DomainFindingLabel> {
        // Describe the free functions that collectively imitate type-owned behavior.
        let mut labels = functions
            .iter()
            .map(|function| DomainFindingLabel {
                span: function.span,
                message: format!(
                    "`{}` contributes behavior for raw `{domain}` text",
                    function.name
                ),
            })
            .collect::<Vec<_>>();

        // Attach corroborating state before stabilizing the diagnostic order.
        labels.extend(fields.into_iter().map(|field| DomainFindingLabel {
            span: field.span,
            message: format!("raw field `{}` stores the same domain concept", field.name),
        }));

        // Identify an already-existing exact domain type independently from raw state.
        labels.extend(
            existing_type
                .into_iter()
                .map(|existing| DomainFindingLabel {
                    span: existing.span,
                    message: format!("type `{}` already names this domain", existing.name),
                }),
        );

        // Stabilize every supporting label at its authored declaration.
        labels.sort_by_key(|label| label.span.lo());
        labels
    }

    /// Builds one repeated-revalidation diagnostic record.
    fn revalidation_finding(
        domain: String,
        evidence: &[&DomainEvidenceRevalidation],
    ) -> DomainFindingRevalidation {
        // Explain every consumer participating in the repeated boundary work.
        let labels = evidence.iter().map(|item| DomainFindingLabel {
            span: item.span,
            message: format!(
                "`{}` re-establishes `{domain}` invariants for `{}`",
                item.consumer.name, item.parameter
            ),
        });
        let labels = labels.collect();

        // Anchor the group at its earliest source-ordered revalidation.
        DomainFindingRevalidation {
            span: evidence[0].span,
            domain,
            labels,
        }
    }

    /// Records existing named types that may already own an inferred domain.
    pub(crate) fn record_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Restrict evidence to authored declarations that can name a domain type.
        let is_type = matches!(
            item.kind,
            ItemKind::Struct(..) | ItemKind::Enum(..) | ItemKind::Union(..) | ItemKind::TyAlias(..)
        );
        if !is_type || item.span.in_external_macro(cx.sess().source_map()) {
            return;
        }
        let Some(ident) = item.kind.ident() else {
            return;
        };

        // Preserve the type and its module for exact domain-name matching.
        let module = cx.tcx.parent_module_from_def_id(item.owner_id.def_id);
        self.types.push(DomainEvidenceType {
            module: module.to_local_def_id(),
            span: ident.span,
            name: ident.name,
        });
    }

    /// Records raw textual struct fields as supporting evidence.
    pub(crate) fn record_field(&mut self, cx: &LateContext<'_>, field: &FieldDef<'_>) {
        // Require authored named state stored directly as textual data.
        if field.is_positional() || field.span.in_external_macro(cx.sess().source_map()) {
            return;
        }
        let owner = cx.tcx.parent(field.def_id.to_def_id());
        let is_struct_field = cx.tcx.def_kind(owner) == DefKind::Struct;
        let ty = cx.tcx.type_of(field.def_id).instantiate_identity();
        if !is_struct_field || !ty.is_textual(cx) {
            return;
        }
        let Some(domain) = field.ident.name.parameter_domain() else {
            return;
        };

        // Attach named textual state to its containing module and inferred domain.
        let module = cx.tcx.parent_module_from_def_id(owner.expect_local());
        self.fields.push(DomainEvidenceField {
            module: module.to_local_def_id(),
            span: field.ident.span,
            name: field.ident.name,
            domain,
        });
    }

    /// Records free-function family and consumer-side revalidation evidence.
    pub(crate) fn record_function<'tcx>(
        &mut self,
        cx: &LateContext<'tcx>,
        signature: &ParameterSignature,
        body: &'tcx Body<'tcx>,
    ) {
        // Classify the declaration, its behavior vocabulary, and its textual inputs.
        let module = cx.tcx.parent_module_from_def_id(signature.def_id);
        let module = module.to_local_def_id();
        let parent_is_module = cx
            .tcx
            .opt_local_parent(signature.def_id)
            .is_some_and(|parent| cx.tcx.def_kind(parent) == DefKind::Mod);
        let textual = Self::textual_parameters(signature);

        // Free functions collectively imitate an impl block only when behavior is visible.
        if parent_is_module && signature.name.has_domain_behavior() {
            for domain in signature.name.function_domains(&textual) {
                self.functions.push(DomainEvidenceFunction {
                    def_id: signature.def_id,
                    module,
                    span: signature.span,
                    name: signature.name,
                    domain,
                });
            }
        }
        if signature.name.establishes_domain_invariant() {
            return;
        }

        // Downstream consumers count when they re-establish an identifiable invariant.
        let revalidated = body.revalidated_bindings(cx, &textual);
        for parameter in textual {
            if !revalidated.contains(&parameter.hir_id) {
                continue;
            }
            let Some(evidence) =
                DomainEvidenceRevalidation::from_parameter(signature, module, parameter)
            else {
                continue;
            };
            self.revalidations.push(evidence);
        }
    }

    /// Finds raw textual fields reinforcing one module-local domain.
    fn matching_fields(&self, key: &DomainEvidenceKey) -> Vec<&DomainEvidenceField> {
        self.fields
            .iter()
            .filter(|field| field.module == key.module && field.domain == key.domain)
            .collect()
    }

    /// Groups only families meeting the function or field evidence threshold.
    fn family_groups(&self) -> HashMap<DomainEvidenceKey, Vec<&DomainEvidenceFunction>> {
        let mut grouped = HashMap::<DomainEvidenceKey, Vec<&DomainEvidenceFunction>>::new();
        for function in &self.functions {
            let key = DomainEvidenceKey {
                module: function.module,
                domain: function.domain.clone(),
            };
            grouped.entry(key).or_default().push(function);
        }
        grouped.retain(|key, functions| {
            let definitions = functions.iter().map(|function| function.def_id);
            let distinct = definitions.collect::<HashSet<_>>().len();
            distinct >= 2 || (distinct == 1 && !self.matching_fields(key).is_empty())
        });
        grouped
    }

    /// Finds an existing type whose exact normalized name matches the inferred domain.
    fn matching_type(&self, key: &DomainEvidenceKey) -> Option<&DomainEvidenceType> {
        self.types.iter().find(|existing| {
            existing.module == key.module
                && identifier_case::words(existing.name.as_str()).concat() == key.domain
        })
    }

    /// Produces one diagnostic per strongly supported free-function family.
    pub(crate) fn family_findings(&self) -> Vec<DomainFindingFamily> {
        let mut findings = Vec::new();
        for (key, functions) in self.family_groups() {
            // Combine behavioral, state, and existing-type evidence into one finding.
            let fields = self.matching_fields(&key);
            let existing_type = self.matching_type(&key);
            let labels = Self::family_labels(&key.domain, &functions, fields, existing_type);

            // Anchor the family at its first behavior and attach corroborating labels.
            findings.push(DomainFindingFamily {
                span: functions[0].span,
                domain: key.domain,
                existing_type: existing_type.map(|existing| existing.name.to_string()),
                labels,
            });
        }
        findings.sort_by_key(|finding| finding.span.lo());
        findings
    }

    /// Groups revalidation evidence by module-local domain.
    fn revalidation_groups(&self) -> HashMap<DomainEvidenceKey, Vec<&DomainEvidenceRevalidation>> {
        let mut grouped = HashMap::new();
        for evidence in &self.revalidations {
            let key = DomainEvidenceKey {
                module: evidence.consumer.module,
                domain: evidence.domain.clone(),
            };
            grouped.entry(key).or_insert_with(Vec::new).push(evidence);
        }
        grouped
    }

    /// Produces repeated-validation findings not superseded by a function family.
    pub(crate) fn revalidation_findings(&self) -> Vec<DomainFindingRevalidation> {
        let family_keys = self.family_groups().into_keys().collect::<HashSet<_>>();
        let grouped = self.revalidation_groups();
        let mut findings = Vec::new();
        for (key, mut evidence) in grouped {
            let definitions = evidence.iter().map(|item| item.consumer.def_id);
            let has_repetition = definitions.collect::<HashSet<_>>().len() >= 2;
            if family_keys.contains(&key) || !has_repetition {
                continue;
            }
            evidence.sort_by_key(|item| item.span.lo());
            findings.push(Self::revalidation_finding(key.domain, &evidence));
        }
        findings.sort_by_key(|finding| finding.span.lo());
        findings
    }

    /// Returns definitions covered by either stronger string-domain diagnostic.
    pub(crate) fn stronger_function_ids(&self) -> HashSet<LocalDefId> {
        let family_groups = self.family_groups();
        let family_keys = family_groups.keys().cloned().collect::<HashSet<_>>();
        let mut definitions = HashSet::new();
        for functions in family_groups.into_values() {
            definitions.extend(functions.into_iter().map(|function| function.def_id));
        }

        // Repeated revalidation is stronger only outside an established function family.
        for (key, evidence) in self.revalidation_groups() {
            let distinct = evidence.iter().map(|item| item.consumer.def_id);
            let distinct = distinct.collect::<HashSet<_>>();
            if family_keys.contains(&key) || distinct.len() < MIN_REVALIDATION_CONSUMER_COUNT {
                continue;
            }
            definitions.extend(distinct);
        }
        definitions
    }
}
