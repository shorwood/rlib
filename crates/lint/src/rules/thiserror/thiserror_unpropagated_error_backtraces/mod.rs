extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use rustc_errors::DiagDecorator;
use rustc_hir::{FieldDef, Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty;
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use super::contracts::{ThiserrorContractCatalog, thiserror_attributes};
use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::authored_item_source;

struct SourceField {
    span: Span,
    name: String,
    target: LocalDefId,
    forwards_backtrace: bool,
}

#[derive(Default)]
struct ErrorShape {
    captures: Vec<Span>,
    sources: Vec<SourceField>,
}

enum ViolationKind {
    DuplicateCapture,
    MissingForwarding { field: String },
}

struct Violation {
    span: Span,
    kind: ViolationKind,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        match &self.kind {
            ViolationKind::DuplicateCapture => Cow::Borrowed(
                "wrapper captures a backtrace despite its source already providing one",
            ),
            ViolationKind::MissingForwarding { field } => Cow::Owned(format!(
                "source field `{field}` does not forward its existing backtrace"
            )),
        }
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        match self.kind {
            ViolationKind::DuplicateCapture => Cow::Borrowed(
                "capturing again at the wrapper duplicates allocation and can hide the original failure location",
            ),
            ViolationKind::MissingForwarding { .. } => Cow::Borrowed(
                "thiserror only delegates generic backtrace requests through a source field marked `#[backtrace]`",
            ),
        }
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        match self.kind {
            ViolationKind::DuplicateCapture => Cow::Borrowed(
                "remove the wrapper capture and mark the source field with `#[backtrace]`",
            ),
            ViolationKind::MissingForwarding { .. } => {
                Cow::Borrowed("mark the source field with `#[backtrace]` to forward the original")
            }
        }
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            THISERROR_UNPROPAGATED_ERROR_BACKTRACES,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "backtrace continuity breaks here");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

#[derive(Default)]
struct ThiserrorUnpropagatedErrorBacktraces {
    catalog: ThiserrorContractCatalog,
    order: Vec<LocalDefId>,
    shapes: HashMap<LocalDefId, ErrorShape>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub THISERROR_UNPROPAGATED_ERROR_BACKTRACES,
    Warn,
    "finds redundant or unpropagated thiserror backtraces",
    ThiserrorUnpropagatedErrorBacktraces::default()
}

impl LateLintPass<'_> for ThiserrorUnpropagatedErrorBacktraces {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
        if item.span.from_expansion() {
            return;
        }
        let hir_fields = match item.kind {
            ItemKind::Struct(_, _, data) => data.fields().iter().collect::<Vec<_>>(),
            _ => return,
        };
        let Some(source) = authored_item_source(cx, item) else {
            return;
        };
        let Ok(structure) = syn::parse_str::<syn::ItemStruct>(&source) else {
            return;
        };
        let syn_fields = structure.fields.into_iter().collect::<Vec<_>>();
        let mut shape = ErrorShape::default();
        for (field, hir_field) in syn_fields.iter().zip(hir_fields) {
            record_field(cx, &mut shape, field, hir_field);
        }
        self.order.push(item.owner_id.def_id);
        self.shapes.insert(item.owner_id.def_id, shape);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for definition in &self.order {
            if self.catalog.derived_type(*definition).is_none() {
                continue;
            }
            let Some(shape) = self.shapes.get(definition) else {
                continue;
            };
            for source in &shape.sources {
                if self.catalog.derived_type(source.target).is_none()
                    || !self.source_provides_backtrace(source.target)
                    || source.forwards_backtrace
                {
                    continue;
                }
                if let Some(span) = shape.captures.first() {
                    Violation {
                        span: *span,
                        kind: ViolationKind::DuplicateCapture,
                    }
                    .emit(cx);
                } else {
                    Violation {
                        span: source.span,
                        kind: ViolationKind::MissingForwarding {
                            field: source.name.clone(),
                        },
                    }
                    .emit(cx);
                }
            }
        }
    }
}

impl ThiserrorUnpropagatedErrorBacktraces {
    fn source_provides_backtrace(&self, definition: LocalDefId) -> bool {
        self.provides_backtrace(definition, &mut HashSet::new())
    }

    fn provides_backtrace(
        &self,
        definition: LocalDefId,
        visiting: &mut HashSet<LocalDefId>,
    ) -> bool {
        if !visiting.insert(definition) {
            return false;
        }
        let result = self.shapes.get(&definition).is_some_and(|shape| {
            !shape.captures.is_empty()
                || shape.sources.iter().any(|source| {
                    source.forwards_backtrace
                        && self.catalog.derived_type(source.target).is_some()
                        && self.provides_backtrace(source.target, visiting)
                })
        });
        visiting.remove(&definition);
        result
    }
}

fn record_field(
    cx: &LateContext<'_>,
    shape: &mut ErrorShape,
    field: &syn::Field,
    hir_field: &FieldDef<'_>,
) {
    let Some(name) = field.ident.as_ref().map(ToString::to_string) else {
        return;
    };
    let attributes = thiserror_attributes(&field.attrs);
    let field_type = cx.tcx.type_of(hir_field.def_id).instantiate_identity();
    let is_backtrace = attributes.backtrace || is_std_backtrace(cx, field_type);
    let is_source = attributes.source || name == "source";
    if is_source {
        if let Some(target) = field_type
            .ty_adt_def()
            .and_then(|definition| definition.did().as_local())
        {
            shape.sources.push(SourceField {
                span: hir_field.span,
                name,
                target,
                forwards_backtrace: is_backtrace,
            });
        }
    } else if is_backtrace {
        shape.captures.push(hir_field.span);
    }
}

fn is_std_backtrace(cx: &LateContext<'_>, field_type: ty::Ty<'_>) -> bool {
    let Some(definition) = field_type.ty_adt_def() else {
        return false;
    };
    cx.tcx.crate_name(definition.did().krate).as_str() == "std"
        && cx.tcx.item_name(definition.did()).as_str() == "Backtrace"
}
