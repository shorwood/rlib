extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::HashMap;

use rustc_errors::DiagDecorator;
use rustc_hir::{ImplItem, ImplItemKind, Item, ItemKind, Node};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty;
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use crate::utils::diagnostic::LateViolation;
use crate::utils::impl_target::ImplTargetExt;

/// Carries the `Violation` state used by this analysis.
struct Violation {
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `name` value used by this analysis.
    name: String,
    /// Stores the `setters` value used by this analysis.
    setters: usize,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "manual builder `{}` has a Bon-compatible structural protocol",
            self.name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "its start method, {} consuming setters, and infallible terminal method reproduce generated builder mechanics",
            self.setters
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "derive `bon::Builder` for structural construction, or put `#[builder]` on the domain constructor when it owns policy",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            BON_MANUAL_BUILDER_IMPLEMENTATIONS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.span,
                    "this authored type duplicates Bon's builder protocol",
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

#[derive(Default)]
/// Carries the `Candidate` state used by this analysis.
struct Candidate {
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `name` value used by this analysis.
    name: String,
    /// Stores the `fields` value used by this analysis.
    fields: usize,
    /// Stores the `has_start` value used by this analysis.
    has_start: bool,
    /// Stores the `setters` value used by this analysis.
    setters: usize,
    /// Stores the `has_terminal` value used by this analysis.
    has_terminal: bool,
}

/// Performs the `is_fallible` step of the lint analysis.
fn is_fallible(cx: &LateContext<'_>, ty: ty::Ty<'_>) -> bool {
    matches!(ty.kind(), ty::Adt(definition, _) if matches!(cx.tcx.item_name(definition.did()).as_str(), "Result" | "Option"))
}

#[derive(Default)]
/// Carries the `BonManualBuilderImplementations` state used by this analysis.
struct BonManualBuilderImplementations {
    /// Stores the `candidates` value used by this analysis.
    candidates: HashMap<LocalDefId, Candidate>,
}

impl BonManualBuilderImplementations {
    /// Minimum structural fields and setters that establish a builder protocol.
    const MINIMUM_STRUCTURAL_MEMBERS: usize = 2;
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub BON_MANUAL_BUILDER_IMPLEMENTATIONS,
    Warn,
    "finds manual structural builders reproducible by Bon",
    BonManualBuilderImplementations::default()
}

impl LateLintPass<'_> for BonManualBuilderImplementations {
    fn check_item(&mut self, _cx: &LateContext<'_>, item: &Item<'_>) {
        // Prepare the values used by this stage.
        let ItemKind::Struct(identifier, _, data) = item.kind else {
            return;
        };
        let name = identifier.name.as_str();
        if item.span.from_expansion()
            || !name.ends_with("Builder")
            || data.fields().len() < Self::MINIMUM_STRUCTURAL_MEMBERS
        {
            return;
        }

        // Update the accumulated analysis state.
        self.candidates.insert(
            item.owner_id.def_id,
            Candidate {
                span: identifier.span,
                name: name.to_owned(),
                fields: data.fields().len(),
                ..Candidate::default()
            },
        );
    }

    fn check_impl_item(&mut self, cx: &LateContext<'_>, item: &ImplItem<'_>) {
        // Prepare the values used by this stage.
        let ImplItemKind::Fn(signature, _) = item.kind else {
            return;
        };
        if item.span.from_expansion() {
            return;
        }
        let implementation = cx.tcx.local_parent(item.owner_id.def_id);

        // Prepare the values used by this stage.
        let Node::Item(parent) = cx.tcx.hir_node_by_def_id(implementation) else {
            return;
        };
        let Some(definition) = parent.direct_struct(cx) else {
            return;
        };

        // Prepare the values used by this stage.
        let Some(analyze_candidate) = self.candidates.get_mut(&definition) else {
            return;
        };

        // Prepare the values used by this stage.
        let function = cx
            .tcx
            .fn_sig(item.owner_id.def_id)
            .instantiate_identity()
            .skip_binder();
        let output_is_builder = matches!(function.output().kind(), ty::Adt(adt, _) if adt.did() == definition.to_def_id());

        // Prepare the values used by this stage.
        let receiver_is_builder = function
            .inputs()
            .first()
            .is_some_and(|receiver| matches!(receiver.kind(), ty::Adt(adt, _) if adt.did() == definition.to_def_id()));
        let name = item.ident.name.as_str();

        // Reject inputs that do not satisfy this stage.
        if !signature.decl.implicit_self.has_implicit_self() {
            analyze_candidate.has_start |=
                name == "new" && function.inputs().is_empty() && output_is_builder;
        } else if receiver_is_builder && function.inputs().len() == 2 && output_is_builder {
            analyze_candidate.setters += 1;
        } else if receiver_is_builder
            && function.inputs().len() == 1
            && matches!(name, "build" | "complete" | "finish")
            && !output_is_builder
            && !is_fallible(cx, function.output())
        {
            analyze_candidate.has_terminal = true;
        }
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for analyze_candidate in self.candidates.values() {
            // Reject inputs that do not satisfy this stage.
            if !(analyze_candidate.has_start
                && analyze_candidate.has_terminal
                && analyze_candidate.setters >= Self::MINIMUM_STRUCTURAL_MEMBERS
                && analyze_candidate.setters >= analyze_candidate.fields)
            {
                continue;
            }

            // Perform the next step of the analysis.
            Violation {
                span: analyze_candidate.span,
                name: analyze_candidate.name.clone(),
                setters: analyze_candidate.setters,
            }
            .emit(cx);
        }
    }
}
