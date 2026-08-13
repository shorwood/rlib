extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use rustc_errors::DiagDecorator;
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty::{self, Ty};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use super::contracts::ThiserrorContractCatalog;
use crate::utils::diagnostic::LateViolation;

/// Tracks whether type traversal has entered a channel payload.
#[derive(Clone, Copy)]
enum ChannelNesting {
    /// The current type is outside a channel payload.
    OutsideChannel,
    /// The current type is inside a channel payload.
    InsideChannel,
}

impl ChannelNesting {
    /// Returns whether traversal is within a channel payload.
    const fn is_inside(self) -> bool {
        matches!(self, Self::InsideChannel)
    }

    /// Advances traversal after inspecting the current nominal type path.
    fn entering(self, path: &str) -> Self {
        if self.is_inside()
            || ["::mpsc::Sender", "::mpsc::SyncSender", "::mpsc::Receiver"]
                .iter()
                .any(|suffix| path.ends_with(suffix))
        {
            Self::InsideChannel
        } else {
            Self::OutsideChannel
        }
    }
}

/// Carries the `ErrorCandidate` state used by this analysis.
struct ErrorCandidate {
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `blockers` value used by this analysis.
    blockers: Vec<String>,
}

/// Carries the `Violation` state used by this analysis.
struct Violation {
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `boundary` value used by this analysis.
    boundary: String,
    /// Stores the `blockers` value used by this analysis.
    blockers: Vec<String>,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("public thiserror type is not thread-safe at a channel boundary")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "public API `{}` transports this error through a channel, but {} {} the required auto traits",
            self.boundary,
            self.blockers.join(", "),
            if self.blockers.len() == 1 {
                "prevents"
            } else {
                "prevent"
            }
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "use thread-safe field representations such as `Arc`, or make the transport boundary explicitly local",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            THISERROR_NON_SEND_SYNC_PUBLIC_ERRORS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.span,
                    "these fields make the transported error local-only",
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

/// Performs the `collect_channel_errors` step of the lint analysis.
fn collect_channel_errors(
    cx: &LateContext<'_>,
    ty: Ty<'_>,
    nesting: ChannelNesting,
    errors: &mut HashSet<LocalDefId>,
) {
    // Prepare the values used by this stage.
    let ty::Adt(definition, arguments) = ty.kind() else {
        return;
    };
    let path = cx.tcx.def_path_str(definition.did());

    // Prepare the values used by this stage.
    let nesting = nesting.entering(&path);
    let channel = nesting.is_inside();
    if channel && let Some(local) = definition.did().as_local() {
        errors.insert(local);
    }

    // Process the candidates handled by this stage.
    for nested in arguments.types() {
        if channel
            && let ty::Adt(nested_definition, _) = nested.kind()
            && let Some(local) = nested_definition.did().as_local()
        {
            errors.insert(local);
        }
        collect_channel_errors(cx, nested, nesting, errors);
    }
}

#[derive(Default)]
/// Carries the `ThiserrorNonSendSyncPublicErrors` state used by this analysis.
struct ThiserrorNonSendSyncPublicErrors {
    /// Stores the `catalog` value used by this analysis.
    catalog: ThiserrorContractCatalog,
    /// Stores the `errors` value used by this analysis.
    errors: HashMap<LocalDefId, ErrorCandidate>,
    /// Stores the `boundaries` value used by this analysis.
    boundaries: HashMap<LocalDefId, String>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub THISERROR_NON_SEND_SYNC_PUBLIC_ERRORS,
    Warn,
    "finds local-only thiserror values exposed through public channels",
    ThiserrorNonSendSyncPublicErrors::default()
}

impl LateLintPass<'_> for ThiserrorNonSendSyncPublicErrors {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Update the accumulated analysis state.
        self.catalog.check_item(cx, item);
        if item.span.from_expansion() {
            return;
        }

        // Classify the current analyze_candidate.
        match item.kind {
            ItemKind::Struct(_, _, data) | ItemKind::Union(_, _, data) => {
                self.record_error(cx, item, data.fields().iter());
            }
            ItemKind::Enum(_, _, definition) => {
                let fields = definition
                    .variants
                    .iter()
                    .flat_map(|variant| variant.data.fields());
                self.record_error(cx, item, fields);
            }
            ItemKind::Fn { .. } if cx.tcx.visibility(item.owner_id.def_id).is_public() => {
                // Prepare the values used by this stage.
                let output = cx
                    .tcx
                    .fn_sig(item.owner_id.def_id)
                    .instantiate_identity()
                    .skip_binder()
                    .output();
                let mut errors = HashSet::new();

                // Perform the next step of the analysis.
                collect_channel_errors(cx, output, ChannelNesting::OutsideChannel, &mut errors);
                for error in errors {
                    self.boundaries
                        .insert(error, cx.tcx.item_name(item.owner_id.def_id).to_string());
                }
            }
            _ => {}
        }
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for (definition, analyze_candidate) in self.errors.drain() {
            // Prepare the values used by this stage.
            let Some(boundary) = self.boundaries.get(&definition) else {
                continue;
            };
            if self.catalog.derived_type(definition).is_none() {
                continue;
            }

            // Perform the next step of the analysis.
            Violation {
                span: analyze_candidate.span,
                boundary: boundary.clone(),
                blockers: analyze_candidate.blockers,
            }
            .emit(cx);
        }
    }
}

impl ThiserrorNonSendSyncPublicErrors {
    /// Performs the `record_error` operation for this value.
    fn record_error<'hir>(
        &mut self,
        cx: &LateContext<'_>,
        item: &Item<'_>,
        fields: impl IntoIterator<Item = &'hir rustc_hir::FieldDef<'hir>>,
    ) {
        // Reject inputs that do not satisfy this stage.
        if !cx.tcx.visibility(item.owner_id.def_id).is_public() {
            return;
        }

        // Prepare the values used by this stage.
        let blockers = fields
            .into_iter()
            .filter_map(|field| {
                let ty = cx.tcx.type_of(field.def_id).instantiate_identity();
                let ty::Adt(definition, _) = ty.kind() else {
                    return None;
                };
                let path = cx.tcx.def_path_str(definition.did());
                let blocked = path.ends_with("::rc::Rc")
                    || path.ends_with("::cell::Cell")
                    || path.ends_with("::cell::RefCell");
                blocked.then(|| format!("`{path}`"))
            })
            .collect::<Vec<_>>();

        // Reject inputs that do not satisfy this stage.
        if blockers.is_empty() {
            return;
        }

        // Update the accumulated analysis state.
        self.errors.insert(
            item.owner_id.def_id,
            ErrorCandidate {
                span: item.span,
                blockers,
            },
        );
    }
}
