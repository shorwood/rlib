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

use super::utils::contracts::{ThiserrorAttributes, ThiserrorContractCatalog};
use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::AuthoredItemSource;

// -----------------------------------------------------------------------------
// Backtrace: Captured and forwarded trace evidence
// -----------------------------------------------------------------------------

/// Source field and its backtrace-forwarding role.
struct BacktraceSourceField {
    /// Source field receiving a forwarding diagnostic.
    span: Span,
    /// Authored source field name.
    name: String,
    /// Local error type stored by the source field.
    target: LocalDefId,
    /// Whether `#[backtrace]` forwards the source's existing trace.
    is_forwarding_backtrace: bool,
}

/// Captured traces and source links declared by one error type.
#[derive(Default)]
struct BacktraceShape {
    /// Fields that capture a new standard backtrace.
    captures: Vec<Span>,
    /// Source fields that may forward an existing trace.
    sources: Vec<BacktraceSourceField>,
}

impl BacktraceShape {
    /// Returns whether a field has the standard `Backtrace` type.
    fn is_std(cx: &LateContext<'_>, field_type: ty::Ty<'_>) -> bool {
        // Nonalgebraic types cannot resolve to the standard backtrace definition.
        let Some(definition) = field_type.ty_adt_def() else {
            return false;
        };
        cx.tcx.crate_name(definition.did().krate).as_str() == "std"
            && cx.tcx.item_name(definition.did()).as_str() == "Backtrace"
    }

    /// Adds one source-level field to this error's backtrace shape.
    fn record_field(
        &mut self,
        cx: &LateContext<'_>,
        index: usize,
        field: &syn::Field,
        hir_field: &FieldDef<'_>,
    ) {
        let name = field
            .ident
            .as_ref()
            .map_or_else(|| index.to_string(), ToString::to_string);
        let attributes = ThiserrorAttributes::from_attributes(&field.attrs);
        let field_type = cx.tcx.type_of(hir_field.def_id).instantiate_identity();
        let is_backtrace = attributes.is_backtrace || Self::is_std(cx, field_type);
        let is_source = attributes.is_source || name == "source";

        if is_source {
            if let Some(target) = field_type
                .ty_adt_def()
                .and_then(|definition| definition.did().as_local())
            {
                self.sources.push(BacktraceSourceField {
                    span: hir_field.span,
                    name,
                    target,
                    is_forwarding_backtrace: is_backtrace,
                });
            }
        } else if is_backtrace {
            self.captures.push(hir_field.span);
        }
    }
}

// -----------------------------------------------------------------------------
// Violation: Broken backtrace continuity
// -----------------------------------------------------------------------------

/// Distinguishes redundant capture from missing source forwarding.
enum ViolationKind {
    /// A wrapper captures again although its source already has a trace.
    DuplicateCapture,
    /// A source has a trace but the wrapper does not forward it.
    MissingForwarding {
        /// Backtrace field that is not forwarded.
        field: String,
    },
}

/// Derived error whose trace capture or forwarding breaks causal continuity.
struct Violation {
    /// Capture or source field receiving the diagnostic.
    span: Span,
    /// Specific continuity failure.
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

// -----------------------------------------------------------------------------
// ThiserrorUnpropagatedErrorBacktraces: Trace continuity policy
// -----------------------------------------------------------------------------

/// Resolves backtrace availability through chains of local derived errors.
#[derive(Default)]
struct ThiserrorUnpropagatedErrorBacktraces {
    /// Local derived error contracts.
    catalog: ThiserrorContractCatalog,
    /// Authored error definitions in stable source order.
    order: Vec<LocalDefId>,
    /// Backtrace shapes indexed by local error definition.
    shapes: HashMap<LocalDefId, Vec<BacktraceShape>>,
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

        // Expanded items provide derive evidence rather than authored field shapes.
        if item.span.from_expansion() {
            return;
        }

        // Missing authored source prevents field-attribute correlation with HIR.
        let Some(source) = AuthoredItemSource::for_item(cx, item) else {
            return;
        };
        let enum_shapes = |definition: &rustc_hir::EnumDef<'_>| {
            let enumeration = match syn::parse_str::<syn::ItemEnum>(&source) {
                Ok(enumeration) => enumeration,
                // Unparseable enum text cannot provide reliable backtrace attributes.
                Err(_error) => return None,
            };
            let shapes = enumeration
                .variants
                .iter()
                .zip(definition.variants)
                .map(|(variant, hir_variant)| {
                    let mut shape = BacktraceShape::default();
                    for (index, (field, hir_field)) in variant
                        .fields
                        .iter()
                        .zip(hir_variant.data.fields())
                        .enumerate()
                    {
                        shape.record_field(cx, index, field, hir_field);
                    }
                    shape
                })
                .collect();
            Some(shapes)
        };
        let shapes = match item.kind {
            ItemKind::Struct(_, _, data) => {
                // Unparseable struct text cannot provide reliable backtrace attributes.
                let Ok(structure) = syn::parse_str::<syn::ItemStruct>(&source) else {
                    return;
                };
                let mut shape = BacktraceShape::default();
                for (index, (field, hir_field)) in
                    structure.fields.iter().zip(data.fields()).enumerate()
                {
                    shape.record_field(cx, index, field, hir_field);
                }
                vec![shape]
            }
            ItemKind::Enum(_, _, definition) => {
                // Invalid enum source cannot contribute a backtrace shape.
                let Some(shapes) = enum_shapes(&definition) else {
                    return;
                };
                shapes
            }
            // Other declarations cannot define nominal thiserror field contracts.
            _ => return,
        };
        self.order.push(item.owner_id.def_id);
        self.shapes.insert(item.owner_id.def_id, shapes);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for definition in &self.order {
            // Types without a generated error contract have no thiserror trace behavior.
            if self.catalog.derived_type(*definition).is_none() {
                continue;
            }

            // Definitions without authored field shapes provide no continuity evidence.
            let Some(shapes) = self.shapes.get(definition) else {
                continue;
            };
            for shape in shapes {
                for source in &shape.sources {
                    if self.catalog.derived_type(source.target).is_none()
                        || !self.source_provides_backtrace(source.target)
                        || source.is_forwarding_backtrace
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
}
impl ThiserrorUnpropagatedErrorBacktraces {
    /// Recursively determines whether an error captures or forwards a trace.
    fn provides_backtrace(
        &self,
        definition: LocalDefId,
        visiting: &mut HashSet<LocalDefId>,
    ) -> bool {
        // A repeated definition closes a source cycle without inventing trace evidence.
        if !visiting.insert(definition) {
            return false;
        }
        let result = self.shapes.get(&definition).is_some_and(|shapes| {
            shapes.iter().any(|shape| {
                !shape.captures.is_empty()
                    || shape.sources.iter().any(|source| {
                        source.is_forwarding_backtrace
                            && self.catalog.derived_type(source.target).is_some()
                            && self.provides_backtrace(source.target, visiting)
                    })
            })
        });
        visiting.remove(&definition);
        result
    }

    /// Starts a cycle-safe backtrace availability query for one source type.
    fn source_provides_backtrace(&self, definition: LocalDefId) -> bool {
        self.provides_backtrace(definition, &mut HashSet::new())
    }
}
