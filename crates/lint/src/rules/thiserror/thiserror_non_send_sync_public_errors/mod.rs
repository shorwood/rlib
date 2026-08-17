extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use rustc_errors::DiagDecorator;
use rustc_hir::{ImplItem, ImplItemKind, Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty::{self, Ty};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use super::utils::contracts::ThiserrorContractCatalog;
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

    /// Enters channel payload traversal when the current type is a channel.
    fn for_definition(self, cx: &LateContext<'_>, definition: ty::AdtDef<'_>) -> Self {
        let path = cx.tcx.def_path_str(definition.did());
        let is_standard_channel = cx.tcx.crate_name(definition.did().krate).as_str() == "std"
            && matches!(
                cx.tcx.item_name(definition.did()).as_str(),
                "Sender" | "SyncSender" | "Receiver"
            )
            && path.contains("::sync::mpsc::");
        if self.is_inside() || is_standard_channel {
            Self::InsideChannel
        } else {
            Self::OutsideChannel
        }
    }
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

impl ThiserrorNonSendSyncPublicErrors {
    /// Collects known representations that prevent a value from implementing `Send`.
    fn collect_send_blockers(cx: &LateContext<'_>, ty: Ty<'_>, blockers: &mut Vec<String>) {
        // Non-aggregate types cannot contain an `Rc` definition or aggregate arguments.
        let ty::Adt(definition, arguments) = ty.kind() else {
            return;
        };
        let path = cx.tcx.def_path_str(definition.did());
        if cx.tcx.crate_name(definition.did().krate).as_str() == "alloc"
            && path.ends_with("::rc::Rc")
        {
            let blocker = format!("`{path}`");
            if !blockers.contains(&blocker) {
                blockers.push(blocker);
            }
        }
        for nested in arguments.types() {
            Self::collect_send_blockers(cx, nested, blockers);
        }
    }

    /// Collects local types nested within channel payloads.
    fn collect_channel_errors(
        cx: &LateContext<'_>,
        ty: Ty<'_>,
        nesting: ChannelNesting,
        errors: &mut HashSet<LocalDefId>,
    ) {
        // Non-aggregate types cannot introduce or contain a channel payload definition.
        let ty::Adt(definition, arguments) = ty.kind() else {
            return;
        };
        let nesting = nesting.for_definition(cx, *definition);
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
            Self::collect_channel_errors(cx, nested, nesting, errors);
        }
    }
}

impl LateLintPass<'_> for ThiserrorNonSendSyncPublicErrors {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);

        // Generated items do not define authored error or channel-boundary contracts.
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
                let name = cx.tcx.item_name(item.owner_id.def_id).to_string();
                self.record_public_channel_boundary(cx, item.owner_id.def_id, &name);
            }
            _ => {}
        }
    }

    fn check_impl_item(&mut self, cx: &LateContext<'_>, item: &ImplItem<'_>) {
        // Generated or non-function associated items cannot define a channel-returning method.
        if item.span.from_expansion() || !matches!(item.kind, ImplItemKind::Fn(..)) {
            return;
        }
        let name = cx.tcx.item_name(item.owner_id.def_id).to_string();
        self.record_public_channel_boundary(cx, item.owner_id.def_id, &name);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        let mut candidates = self.errors.drain().collect::<Vec<_>>();
        candidates.sort_by_key(|(_, candidate)| candidate.span.lo());
        for (definition, candidate) in candidates {
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
    fn record_public_channel_boundary(
        &mut self,
        cx: &LateContext<'_>,
        definition: LocalDefId,
        name: &str,
    ) {
        // Non-exported functions do not expose channel payloads through public API.
        if !cx.tcx.effective_visibilities(()).is_exported(definition) {
            return;
        }
        let output = cx
            .tcx
            .fn_sig(definition)
            .instantiate_identity()
            .skip_binder()
            .output();
        let mut errors = HashSet::new();
        Self::collect_channel_errors(cx, output, ChannelNesting::OutsideChannel, &mut errors);
        for error in errors {
            self.boundaries.insert(error, name.to_owned());
        }
    }

    /// Records public errors containing known local-only field representations.
    fn record_error<'hir>(
        &mut self,
        cx: &LateContext<'_>,
        item: &Item<'_>,
        fields: impl IntoIterator<Item = &'hir rustc_hir::FieldDef<'hir>>,
    ) {
        // Non-exported error types cannot violate the public transport contract.
        if !cx
            .tcx
            .effective_visibilities(())
            .is_exported(item.owner_id.def_id)
        {
            return;
        }

        let mut blockers = Vec::new();
        for field in fields {
            Self::collect_send_blockers(
                cx,
                cx.tcx.type_of(field.def_id).instantiate_identity(),
                &mut blockers,
            );
        }

        // Errors without a known thread-safety blocker require no boundary correlation.
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
