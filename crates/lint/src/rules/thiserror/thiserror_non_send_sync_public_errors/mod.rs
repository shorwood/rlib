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

// -----------------------------------------------------------------------------
// Violation: Local-only error crossing a public channel
// -----------------------------------------------------------------------------

/// Public thiserror type transported where `Send` and `Sync` are expected.
struct Violation {
    /// Error declaration receiving the diagnostic.
    span: Span,
    /// Public function exposing the channel transport.
    boundary: String,
    /// Field representations preventing thread-safe transport.
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

// -----------------------------------------------------------------------------
// ChannelNesting: Transport payload traversal
// -----------------------------------------------------------------------------

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

/// Collects local types nested within channel payloads.
fn channel_nesting_collect_errors(
    cx: &LateContext<'_>,
    ty: Ty<'_>,
    nesting: ChannelNesting,
    errors: &mut HashSet<LocalDefId>,
) {
    let ty::Adt(definition, arguments) = ty.kind() else {
        return;
    };
    let path = cx.tcx.def_path_str(definition.did());

    let nesting = nesting.entering(&path);
    let channel = nesting.is_inside();
    if channel && let Some(local) = definition.did().as_local() {
        errors.insert(local);
    }

    for nested in arguments.types() {
        if channel
            && let ty::Adt(nested_definition, _) = nested.kind()
            && let Some(local) = nested_definition.did().as_local()
        {
            errors.insert(local);
        }
        channel_nesting_collect_errors(cx, nested, nesting, errors);
    }
}

// -----------------------------------------------------------------------------
// ThreadSafetyCandidate: Blocking field evidence
// -----------------------------------------------------------------------------

/// Public error and the fields preventing thread-safe transport.
struct ThreadSafetyCandidate {
    /// Error declaration receiving a later diagnostic.
    span: Span,
    /// Local-only field representations.
    blockers: Vec<String>,
}

// -----------------------------------------------------------------------------
// ThiserrorNonSendSyncPublicErrors: Public channel safety policy
// -----------------------------------------------------------------------------

/// Correlates local-only public errors with channel-returning APIs.
#[derive(Default)]
struct ThiserrorNonSendSyncPublicErrors {
    /// Local derived error contracts.
    catalog: ThiserrorContractCatalog,
    /// Public errors with fields that block thread-safe transport.
    errors: HashMap<LocalDefId, ThreadSafetyCandidate>,
    /// Public channel-returning functions indexed by transported error type.
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
        self.catalog.check_item(cx, item);
        if item.span.from_expansion() {
            return;
        }

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
                self.record_public_channel_boundary(cx, item);
            }
            _ => {}
        }
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for (definition, candidate) in self.errors.drain() {
            let Some(boundary) = self.boundaries.get(&definition) else {
                continue;
            };
            if self.catalog.derived_type(definition).is_none() {
                continue;
            }

            Violation {
                span: candidate.span,
                boundary: boundary.clone(),
                blockers: candidate.blockers,
            }
            .emit(cx);
        }
    }
}

impl ThiserrorNonSendSyncPublicErrors {
    /// Records local errors transported by one public channel-returning function.
    fn record_public_channel_boundary(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        let output = cx
            .tcx
            .fn_sig(item.owner_id.def_id)
            .instantiate_identity()
            .skip_binder()
            .output();
        let mut errors = HashSet::new();
        channel_nesting_collect_errors(cx, output, ChannelNesting::OutsideChannel, &mut errors);
        for error in errors {
            self.boundaries
                .insert(error, cx.tcx.item_name(item.owner_id.def_id).to_string());
        }
    }

    /// Records public errors containing known local-only field representations.
    fn record_error<'hir>(
        &mut self,
        cx: &LateContext<'_>,
        item: &Item<'_>,
        fields: impl IntoIterator<Item = &'hir rustc_hir::FieldDef<'hir>>,
    ) {
        if !cx.tcx.visibility(item.owner_id.def_id).is_public() {
            return;
        }

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

        if blockers.is_empty() {
            return;
        }

        self.errors.insert(
            item.owner_id.def_id,
            ThreadSafetyCandidate {
                span: item.span,
                blockers,
            },
        );
    }
}
