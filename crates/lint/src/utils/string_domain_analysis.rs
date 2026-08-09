extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_span;

use std::collections::{HashMap, HashSet};

use rustc_hir::def::{DefKind, Res};
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{Body, Expr, ExprKind, FieldDef, HirId, Item, ItemKind};
use rustc_lint::{LateContext, LintContext};
use rustc_span::def_id::LocalDefId;
use rustc_span::{Span, Symbol};

use super::identifier_case::identifier_words;
use super::parameter_analysis::{
    Parameter, ParameterKind, ParameterSignature, parameter_type_is_textual,
};

// -----------------------------------------------------------------------------
// Domain: String domain evidence and inference
// -----------------------------------------------------------------------------

/// Supporting declaration attached to a domain-boundary diagnostic.
pub struct DomainLabel {
    /// Declaration source range.
    pub span: Span,
    /// Explanation of the declaration's contribution to the finding.
    pub message: String,
}

/// Stable key for evidence belonging to one module-local domain.
#[derive(Clone, Eq, Hash, PartialEq)]
struct DomainKey {
    /// Module containing the evidence.
    module: LocalDefId,
    /// Inferred `PascalCase` domain concept.
    domain: String,
}

/// Free functions and raw fields collectively behaving like a missing domain type.
pub struct DomainFamilyFinding {
    /// Primary source range for the family-level warning.
    pub span: Span,
    /// Inferred `PascalCase` domain concept.
    pub domain: String,
    /// Existing same-module type already naming the inferred concept.
    pub existing_type: Option<String>,
    /// Supporting declarations in source order.
    pub labels: Vec<DomainLabel>,
}

/// Raw textual parameters that repeatedly re-establish one domain invariant.
pub struct DomainRevalidationFinding {
    /// Primary parameter source range.
    pub span: Span,
    /// Inferred `PascalCase` domain concept.
    pub domain: String,
    /// Consumer parameters that repeat invariant establishment.
    pub labels: Vec<DomainLabel>,
}

/// One free function contributing behavior to an inferred string domain.
#[derive(Clone)]
struct DomainFunctionEvidence {
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

/// One raw textual field reinforcing an inferred string domain.
#[derive(Clone)]
struct DomainFieldEvidence {
    /// Containing source module.
    module: LocalDefId,
    /// Field name span.
    span: Span,
    /// Authored field name.
    name: Symbol,
    /// Inferred domain concept.
    domain: String,
}

/// One existing type whose name already expresses a candidate domain concept.
struct DomainTypeEvidence {
    /// Containing source module.
    module: LocalDefId,
    /// Type name span.
    span: Span,
    /// Authored type name.
    name: Symbol,
}

/// Identity and module placement of one raw-string consumer.
#[derive(Clone)]
struct DomainConsumer {
    /// Consumer definition identity.
    def_id: LocalDefId,
    /// Containing source module.
    module: LocalDefId,
    /// Authored consumer name.
    name: Symbol,
}

/// One consumer that establishes an invariant on a raw string parameter.
#[derive(Clone)]
struct DomainRevalidationEvidence {
    /// Identity and placement of the consuming function.
    consumer: DomainConsumer,
    /// Parameter name span.
    span: Span,
    /// Authored parameter name.
    parameter: Symbol,
    /// Inferred domain concept.
    domain: String,
}

/// Collects local bindings referenced by one expression.
struct DomainBindingCollector<'analysis, 'tcx> {
    /// Compiler context used to resolve paths.
    cx: &'analysis LateContext<'tcx>,
    /// Resolved local binding identities.
    bindings: HashSet<HirId>,
}

impl<'tcx> Visitor<'tcx> for DomainBindingCollector<'_, 'tcx> {
    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        if let ExprKind::Path(path) = expression.kind
            && let Res::Local(binding) = self.cx.qpath_res(&path, expression.hir_id)
        {
            self.bindings.insert(binding);
        }
        intravisit::walk_expr(self, expression);
    }
}

/// Finds returns without descending into nested closures.
#[derive(Default)]
struct DomainReturnFinder {
    /// Whether a return expression was encountered.
    has_return: bool,
}

impl<'tcx> Visitor<'tcx> for DomainReturnFinder {
    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        if matches!(expression.kind, ExprKind::Ret(_)) {
            self.has_return = true;
            return;
        }
        if matches!(expression.kind, ExprKind::Closure(_)) {
            return;
        }
        intravisit::walk_expr(self, expression);
    }
}

/// Operations conventionally owned by a parsed or validated domain type.
const DOMAIN_BEHAVIOR_WORDS: &[&str] = &[
    "canonicalize",
    "check",
    "compare",
    "convert",
    "decode",
    "encode",
    "format",
    "normalize",
    "parse",
    "path",
    "render",
    "sanitize",
    "validate",
];

/// Normalizes one Rust identifier into lowercase semantic words.
fn domain_normalized_words(identifier: Symbol) -> Vec<String> {
    let words = identifier_words(identifier.as_str());
    words.into_iter().map(|word| word.to_lowercase()).collect()
}

/// Returns whether a word describes behavior conventionally owned by a domain type.
fn domain_word_is_behavior(word: &str) -> bool {
    DOMAIN_BEHAVIOR_WORDS.contains(&word)
}

/// Returns whether a word describes establishment of a domain invariant.
fn domain_word_establishes_invariant(word: &str) -> bool {
    matches!(
        word,
        "canonicalize" | "check" | "normalize" | "parse" | "sanitize" | "validate"
    )
}

/// Returns whether an identifier names an invariant-establishing operation.
fn domain_words_establish_invariant(identifier: Symbol) -> bool {
    let words = domain_normalized_words(identifier);
    words
        .iter()
        .any(|word| domain_word_establishes_invariant(word))
}

/// Finds parameters used by validation calls or early rejection guards.
struct DomainRevalidationVisitor<'analysis, 'tcx> {
    /// Compiler context used to resolve call targets and bindings.
    cx: &'analysis LateContext<'tcx>,
    /// Textual parameter bindings under analysis.
    parameters: HashSet<HirId>,
    /// Parameters for which invariant establishment was observed.
    revalidated: HashSet<HirId>,
}

impl<'analysis, 'tcx> DomainRevalidationVisitor<'analysis, 'tcx> {
    /// Traverses one function body and returns every revalidated textual binding.
    fn analyze(
        cx: &'analysis LateContext<'tcx>,
        body: &'tcx Body<'tcx>,
        parameters: &[&Parameter],
    ) -> HashSet<HirId> {
        // Seed a visitor with the exact textual bindings under analysis.
        let bindings = parameters.iter().map(|parameter| parameter.hir_id);
        let mut visitor = Self {
            cx,
            parameters: bindings.collect(),
            revalidated: HashSet::new(),
        };

        // Traverse the authored body and return the accumulated binding identities.
        visitor.visit_expr(body.value);
        visitor.revalidated
    }

    /// Returns whether an expression contains an explicit function return.
    fn contains_return(expression: &'tcx Expr<'tcx>) -> bool {
        let mut finder = DomainReturnFinder::default();
        finder.visit_expr(expression);
        finder.has_return
    }

    /// Records parameter bindings used anywhere inside an expression.
    fn record_bindings(&mut self, expression: &'tcx Expr<'tcx>) {
        let mut collector = DomainBindingCollector {
            cx: self.cx,
            bindings: HashSet::new(),
        };
        collector.visit_expr(expression);
        let used_parameters = collector.bindings.intersection(&self.parameters).copied();
        self.revalidated.extend(used_parameters);
    }

    /// Records bindings passed to one invariant-establishing free function.
    fn record_call(&mut self, callee: &'tcx Expr<'tcx>, arguments: &'tcx [Expr<'tcx>]) {
        let ExprKind::Path(path) = callee.kind else {
            return;
        };
        let Res::Def(_, def_id) = self.cx.qpath_res(&path, callee.hir_id) else {
            return;
        };
        let name = self.cx.tcx.item_name(def_id);
        if !domain_words_establish_invariant(name) {
            return;
        }
        for argument in arguments {
            self.record_bindings(argument);
        }
    }

    /// Records bindings passed to one invariant-establishing method.
    fn record_method_call(
        &mut self,
        name: Symbol,
        receiver: &'tcx Expr<'tcx>,
        arguments: &'tcx [Expr<'tcx>],
    ) {
        if !domain_words_establish_invariant(name) {
            return;
        }
        self.record_bindings(receiver);
        for argument in arguments {
            self.record_bindings(argument);
        }
    }
}

impl<'tcx> Visitor<'tcx> for DomainRevalidationVisitor<'_, 'tcx> {
    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        // Recognize explicit validation operations and rejection guards.
        match expression.kind {
            ExprKind::Call(callee, arguments) => self.record_call(callee, arguments),
            ExprKind::MethodCall(segment, receiver, arguments, _) => {
                self.record_method_call(segment.ident.name, receiver, arguments);
            }
            ExprKind::If(condition, then, _) if Self::contains_return(then) => {
                self.record_bindings(condition);
            }
            _ => {}
        }
        intravisit::walk_expr(self, expression);
    }
}

/// Removes action and generic transport vocabulary from a domain candidate.
fn domain_meaningful_words(identifier: Symbol) -> Vec<String> {
    let words = domain_normalized_words(identifier);
    words
        .into_iter()
        .filter(|word| {
            !domain_word_is_behavior(word)
                && !matches!(
                    word.as_str(),
                    "contents"
                        | "input"
                        | "left"
                        | "message"
                        | "output"
                        | "raw"
                        | "right"
                        | "source"
                        | "text"
                        | "value"
                )
        })
        .collect()
}

/// Renders lowercase domain words as one `PascalCase` type name.
fn domain_pascal_case(words: impl IntoIterator<Item = String>) -> String {
    words
        .into_iter()
        .map(|word| {
            let mut characters = word.chars();
            characters.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(characters).collect::<String>()
            })
        })
        .collect()
}

/// Infers a domain concept from a precise parameter or field name.
fn domain_from_parameter(identifier: Symbol) -> Option<String> {
    let words = domain_meaningful_words(identifier);
    (!words.is_empty()).then(|| domain_pascal_case(words))
}

/// Returns a parameter-derived domain supported by one operation vocabulary.
fn domain_supported_parameter(
    function_words: &HashSet<String>,
    parameter: &Parameter,
) -> Option<String> {
    // Require the parameter's semantic tokens to occur in the operation name.
    let parameter_words = domain_meaningful_words(parameter.name);
    let is_supported = !parameter_words.is_empty()
        && parameter_words
            .iter()
            .all(|word| function_words.contains(word));

    // Convert only supported, nonempty parameter vocabulary into a domain name.
    is_supported
        .then(|| domain_from_parameter(parameter.name))
        .flatten()
}

/// Connects function vocabulary and textual parameter names to domain candidates.
fn domain_function_names(function: Symbol, parameters: &[&Parameter]) -> HashSet<String> {
    // Derive the domain expressed directly by the operation name.
    let function_words = domain_meaningful_words(function);
    let function_domain =
        (!function_words.is_empty()).then(|| domain_pascal_case(function_words.clone()));
    let function_word_set = function_words.into_iter().collect::<HashSet<_>>();

    // Prefer domains supported by both the operation and a parameter name.
    let mut domains = HashSet::new();
    for parameter in parameters {
        let Some(domain) = domain_supported_parameter(&function_word_set, parameter) else {
            continue;
        };
        domains.insert(domain);
    }

    // A unary behavior function may express its domain entirely in its name.
    if domains.is_empty()
        && parameters.len() == 1
        && let Some(domain) = function_domain
    {
        domains.insert(domain);
    }
    domains
}

/// Selects textual parameters from one analyzed signature.
fn domain_textual_parameters(signature: &ParameterSignature) -> Vec<&Parameter> {
    signature
        .parameters
        .iter()
        .filter(|parameter| parameter.interchangeable == Some(ParameterKind::Text))
        .collect()
}

/// Builds one revalidation record when a parameter exposes a precise domain.
fn domain_revalidation_evidence(
    signature: &ParameterSignature,
    module: LocalDefId,
    parameter: &Parameter,
) -> Option<DomainRevalidationEvidence> {
    // Resolve the domain and preserve the consuming function as one semantic identity.
    let domain = domain_from_parameter(parameter.name)?;
    let consumer = DomainConsumer {
        def_id: signature.def_id,
        module,
        name: signature.name,
    };

    // Keep the parameter site and vocabulary beside its consuming function identity.
    Some(DomainRevalidationEvidence {
        consumer,
        span: parameter.span,
        parameter: parameter.name,
        domain,
    })
}

/// Builds one repeated-revalidation diagnostic record.
fn domain_revalidation_finding(
    domain: String,
    evidence: &[&DomainRevalidationEvidence],
) -> DomainRevalidationFinding {
    // Explain every consumer participating in the repeated boundary work.
    let labels = evidence.iter().map(|item| DomainLabel {
        span: item.span,
        message: format!(
            "`{}` re-establishes `{domain}` invariants for `{}`",
            item.consumer.name, item.parameter
        ),
    });
    let labels = labels.collect();

    // Anchor the group at its earliest source-ordered revalidation.
    DomainRevalidationFinding {
        span: evidence[0].span,
        domain,
        labels,
    }
}

/// Builds source-ordered labels supporting one function-family finding.
fn domain_family_labels(
    domain: &str,
    functions: &[&DomainFunctionEvidence],
    fields: Vec<&DomainFieldEvidence>,
    existing_type: Option<&DomainTypeEvidence>,
) -> Vec<DomainLabel> {
    // Describe the free functions that collectively imitate type-owned behavior.
    let mut labels = functions
        .iter()
        .map(|function| DomainLabel {
            span: function.span,
            message: format!(
                "`{}` contributes behavior for raw `{domain}` text",
                function.name
            ),
        })
        .collect::<Vec<_>>();

    // Attach corroborating state and any already-existing exact domain type.
    labels.extend(fields.into_iter().map(|field| DomainLabel {
        span: field.span,
        message: format!("raw field `{}` stores the same domain concept", field.name),
    }));
    labels.extend(existing_type.into_iter().map(|existing| DomainLabel {
        span: existing.span,
        message: format!("type `{}` already names this domain", existing.name),
    }));
    labels.sort_by_key(|label| label.span.lo());
    labels
}

/// Complete crate-local evidence used by the string-domain lints.
#[derive(Default)]
pub struct DomainAnalyzer {
    /// Free-function behavior grouped later by module and domain.
    functions: Vec<DomainFunctionEvidence>,
    /// Raw textual fields grouped later by module and domain.
    fields: Vec<DomainFieldEvidence>,
    /// Existing module-local types used to avoid proposing duplicates.
    types: Vec<DomainTypeEvidence>,
    /// Consumer-side invariant establishment.
    revalidations: Vec<DomainRevalidationEvidence>,
}

impl DomainAnalyzer {
    /// Records existing named types that may already own an inferred domain.
    pub fn record_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
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
        self.types.push(DomainTypeEvidence {
            module: module.to_local_def_id(),
            span: ident.span,
            name: ident.name,
        });
    }

    /// Records raw textual struct fields as supporting evidence.
    pub fn record_field(&mut self, cx: &LateContext<'_>, field: &FieldDef<'_>) {
        // Require authored named state stored directly as textual data.
        if field.is_positional() || field.span.in_external_macro(cx.sess().source_map()) {
            return;
        }
        let owner = cx.tcx.parent(field.def_id.to_def_id());
        let is_struct_field = cx.tcx.def_kind(owner) == DefKind::Struct;
        let ty = cx.tcx.type_of(field.def_id).instantiate_identity();
        if !is_struct_field || !parameter_type_is_textual(cx, ty) {
            return;
        }
        let Some(domain) = domain_from_parameter(field.ident.name) else {
            return;
        };

        // Attach named textual state to its containing module and inferred domain.
        let module = cx.tcx.parent_module_from_def_id(owner.expect_local());
        self.fields.push(DomainFieldEvidence {
            module: module.to_local_def_id(),
            span: field.ident.span,
            name: field.ident.name,
            domain,
        });
    }

    /// Records free-function family and consumer-side revalidation evidence.
    pub fn record_function<'tcx>(
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

        // Identify behavior, boundary establishment, and relevant input bindings.
        let function_words = domain_normalized_words(signature.name);
        let has_domain_behavior = function_words
            .iter()
            .any(|word| domain_word_is_behavior(word));
        let establishes_invariant = domain_words_establish_invariant(signature.name);
        let textual = domain_textual_parameters(signature);

        // Free functions collectively imitate an impl block only when behavior is visible.
        if parent_is_module && has_domain_behavior {
            for domain in domain_function_names(signature.name, &textual) {
                self.functions.push(DomainFunctionEvidence {
                    def_id: signature.def_id,
                    module,
                    span: signature.span,
                    name: signature.name,
                    domain,
                });
            }
        }
        if establishes_invariant {
            return;
        }

        // Downstream consumers count when they re-establish an identifiable invariant.
        let revalidated = DomainRevalidationVisitor::analyze(cx, body, &textual);
        for parameter in textual {
            if !revalidated.contains(&parameter.hir_id) {
                continue;
            }
            let Some(evidence) = domain_revalidation_evidence(signature, module, parameter) else {
                continue;
            };
            self.revalidations.push(evidence);
        }
    }

    /// Finds raw textual fields reinforcing one module-local domain.
    fn matching_fields(&self, key: &DomainKey) -> Vec<&DomainFieldEvidence> {
        self.fields
            .iter()
            .filter(|field| field.module == key.module && field.domain == key.domain)
            .collect()
    }

    /// Groups only families meeting the function or field evidence threshold.
    fn family_groups(&self) -> HashMap<DomainKey, Vec<&DomainFunctionEvidence>> {
        let mut grouped = HashMap::<DomainKey, Vec<&DomainFunctionEvidence>>::new();
        for function in &self.functions {
            let key = DomainKey {
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
    fn matching_type(&self, key: &DomainKey) -> Option<&DomainTypeEvidence> {
        self.types.iter().find(|existing| {
            existing.module == key.module
                && identifier_words(existing.name.as_str()).concat() == key.domain
        })
    }

    /// Produces one diagnostic per strongly supported free-function family.
    pub fn family_findings(&self) -> Vec<DomainFamilyFinding> {
        let mut findings = Vec::new();
        for (key, functions) in self.family_groups() {
            // Combine behavioral, state, and existing-type evidence into one finding.
            let fields = self.matching_fields(&key);
            let existing_type = self.matching_type(&key);
            let labels = domain_family_labels(&key.domain, &functions, fields, existing_type);

            // Anchor the family at its first behavior and attach corroborating labels.
            findings.push(DomainFamilyFinding {
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
    fn revalidation_groups(&self) -> HashMap<DomainKey, Vec<&DomainRevalidationEvidence>> {
        let mut grouped = HashMap::new();
        for evidence in &self.revalidations {
            let key = DomainKey {
                module: evidence.consumer.module,
                domain: evidence.domain.clone(),
            };
            grouped.entry(key).or_insert_with(Vec::new).push(evidence);
        }
        grouped
    }

    /// Produces repeated-validation findings not superseded by a function family.
    pub fn revalidation_findings(&self) -> Vec<DomainRevalidationFinding> {
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
            findings.push(domain_revalidation_finding(key.domain, &evidence));
        }
        findings.sort_by_key(|finding| finding.span.lo());
        findings
    }

    /// Returns definitions covered by either stronger string-domain diagnostic.
    pub fn stronger_function_ids(&self) -> HashSet<LocalDefId> {
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
            if family_keys.contains(&key) || distinct.len() < 2 {
                continue;
            }
            definitions.extend(distinct);
        }
        definitions
    }
}

#[cfg(test)]
mod tests {
    use super::rustc_span::{Symbol, create_default_session_globals_then};
    use super::{domain_from_parameter, domain_meaningful_words};

    #[test]
    fn extracts_domain_names_from_precise_parameters() {
        create_default_session_globals_then(|| {
            assert_eq!(
                domain_from_parameter(Symbol::intern("left_access_token")),
                Some("AccessToken".to_owned())
            );
            assert_eq!(domain_from_parameter(Symbol::intern("source")), None);
        });
    }

    #[test]
    fn removes_behavior_and_transport_vocabulary() {
        create_default_session_globals_then(|| {
            assert_eq!(
                domain_meaningful_words(Symbol::intern("normalize_project_slug")),
                ["project", "slug"]
            );
            assert!(domain_meaningful_words(Symbol::intern("format_message")).is_empty());
        });
    }
}
