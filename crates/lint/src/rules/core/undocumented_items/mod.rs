extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{FieldDef, HirId, ImplItem, Item, ItemKind, Node, TraitItem, Variant};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::{Pos, Span};

use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::{FieldProvenanceExt, SpanProvenanceExt};

// -----------------------------------------------------------------------------
// Violation: Undocumented declaration diagnostic
// -----------------------------------------------------------------------------

/// Authored semantic declaration with no documentation contract.
struct Violation {
    /// Declaration node used for item-level lint attributes.
    hir_id: HirId,
    /// Identifier or declaration extent receiving the diagnostic.
    span: Span,
    /// User-facing declaration kind.
    kind: &'static str,
}

impl Violation {
    /// Classifies an authored declaration only when its semantic contract is undocumented.
    fn from_declaration(
        cx: &LateContext<'_>,
        hir_id: HirId,
        span: Span,
        kind: &'static str,
    ) -> Option<Self> {
        // Documented, external, and generated declarations do not need an authored diagnostic.
        if Self::has_documentation(cx, hir_id)
            || span.in_external_macro(cx.sess().source_map())
            || span.is_build_generated(cx)
        {
            return None;
        }
        Some(Self { hir_id, span, kind })
    }

    /// Returns whether an item carries any authored `doc` attribute or documentation comment.
    fn has_documentation(cx: &LateContext<'_>, hir_id: HirId) -> bool {
        cx.tcx.hir_attrs(hir_id).iter().any(|attribute| {
            attribute
                .doc_str()
                .is_some_and(|documentation| !documentation.as_str().trim().is_empty())
        })
    }
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!("this {} is missing documentation", self.kind))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "without documentation, readers and agents must infer this {}'s purpose, invariants, and contract from implementation details",
            self.kind
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("document its purpose, contract, or semantic role")
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            UNDOCUMENTED_ITEMS,
            self.hir_id,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// UndocumentedItems: Declaration contract documentation policy
// -----------------------------------------------------------------------------

/// Late lint pass that requires semantic declarations to explain their public and internal roles.
struct UndocumentedItems;

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub UNDOCUMENTED_ITEMS,
    Warn,
    "requires documentation for semantic declarations while leaving barrel modules clean",
    UndocumentedItems
}

impl UndocumentedItems {
    /// Returns whether a function is an executable test case rather than supporting test code.
    fn is_test_case(cx: &LateContext<'_>, item: &Item<'_>) -> bool {
        // Only function items in a test-harness compilation can be executable test cases.
        if !cx.sess().opts.test || !matches!(item.kind, ItemKind::Fn { .. }) {
            return false;
        }

        // The built-in attribute expands away from the original function before HIR lowering.
        let source_file = cx.sess().source_map().lookup_source_file(item.span.lo());

        // Files without retained source cannot prove an authored test attribute.
        let Some(source) = source_file.src.as_deref() else {
            return false;
        };

        // Invalid byte offsets cannot delimit the attributes preceding this function.
        let Ok(offset) = usize::try_from((item.span.lo() - source_file.start_pos).to_u32()) else {
            return false;
        };

        // A missing source prefix leaves no authored attribute text to inspect.
        let Some(prefix) = source.get(..offset) else {
            return false;
        };
        for line in prefix.lines().rev() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            // The nearest built-in test attribute classifies this function as a test case.
            if line == "#[test]" {
                return true;
            }
            if line.starts_with("#[") {
                continue;
            }
            break;
        }
        false
    }

    /// Returns whether a declaration belongs directly to a crate or named module.
    fn is_module_level(cx: &LateContext<'_>, hir_id: HirId) -> bool {
        let owner = cx.tcx.hir_get_parent_item(hir_id);
        let owner_hir_id = cx.tcx.local_def_id_to_hir_id(owner.def_id);
        matches!(
            cx.tcx.parent_hir_node(owner_hir_id),
            Node::Crate(_)
                | Node::Item(Item {
                    kind: ItemKind::Mod(..),
                    ..
                })
        )
    }

    /// Classifies module-level declarations governed by this rule.
    fn item_kind(item: &Item<'_>) -> Option<&'static str> {
        Self::type_item_kind(item).or_else(|| Self::value_item_kind(item))
    }

    /// Classifies documented nominal type declarations.
    fn type_item_kind(item: &Item<'_>) -> Option<&'static str> {
        Self::concrete_type_item_kind(item).or_else(|| Self::abstract_type_item_kind(item))
    }

    /// Classifies concrete nominal declarations.
    const fn concrete_type_item_kind(item: &Item<'_>) -> Option<&'static str> {
        match item.kind {
            ItemKind::Enum(..) => Some("enum"),
            ItemKind::Struct(..) => Some("struct"),
            ItemKind::Union(..) => Some("union"),
            _ => None,
        }
    }

    /// Classifies abstract nominal declarations.
    const fn abstract_type_item_kind(item: &Item<'_>) -> Option<&'static str> {
        match item.kind {
            ItemKind::Trait(..) => Some("trait"),
            ItemKind::TraitAlias(..) => Some("trait alias"),
            ItemKind::TyAlias(..) => Some("type alias"),
            _ => None,
        }
    }

    /// Classifies documented value-level declarations.
    const fn value_item_kind(item: &Item<'_>) -> Option<&'static str> {
        match item.kind {
            ItemKind::Const(..) => Some("constant"),
            ItemKind::Fn { .. } => Some("function"),
            ItemKind::Static(..) => Some("static"),
            _ => None,
        }
    }

    /// Returns the identifier span for a named module-level declaration.
    fn item_span(item: &Item<'_>) -> Span {
        item.kind
            .ident()
            .map_or(item.span, |identifier| identifier.span)
    }

    /// Returns whether an associated item belongs to a trait implementation.
    fn is_trait_implementation_item(cx: &LateContext<'_>, item: &ImplItem<'_>) -> bool {
        // Resolve the associated item's owning HIR declaration.
        let parent = cx.tcx.local_parent(item.owner_id.def_id);
        let parent_node = cx.tcx.hir_node_by_def_id(parent);

        // Resolve the enclosing item and its implementation declaration.
        let Node::Item(parent_item) = parent_node else {
            return false;
        };

        // Only implementation parents can establish trait-implementation ownership.
        let ItemKind::Impl(implementation) = parent_item.kind else {
            return false;
        };

        // Exclude associated items implementing an external trait contract.
        implementation.of_trait.is_some()
    }
}

impl<'tcx> LateLintPass<'tcx> for UndocumentedItems {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        // Nested declarations and executable test cases are outside this documentation policy.
        if !Self::is_module_level(cx, item.hir_id()) || Self::is_test_case(cx, item) {
            return;
        }

        // Unclassified item kinds do not carry a semantic documentation contract here.
        let Some(kind) = Self::item_kind(item) else {
            return;
        };

        // Already documented or generated declarations produce no violation.
        let Some(violation) =
            Violation::from_declaration(cx, item.hir_id(), Self::item_span(item), kind)
        else {
            return;
        };
        violation.emit(cx);
    }

    fn check_field_def(&mut self, cx: &LateContext<'tcx>, field: &'tcx FieldDef<'tcx>) {
        // Nested or framework-generated fields do not need authored field documentation here.
        if !Self::is_module_level(cx, field.hir_id) || field.is_framework_generated(cx) {
            return;
        }
        let span = if field.is_positional() {
            field.span
        } else {
            field.ident.span
        };

        // Documented or generated fields produce no violation.
        let Some(violation) = Violation::from_declaration(cx, field.hir_id, span, "field") else {
            return;
        };
        violation.emit(cx);
    }

    fn check_variant(&mut self, cx: &LateContext<'tcx>, variant: &'tcx Variant<'tcx>) {
        // Variants outside module-level nominal declarations are outside this policy.
        if !Self::is_module_level(cx, variant.hir_id) {
            return;
        }

        // Documented or generated variants produce no violation.
        let Some(violation) =
            Violation::from_declaration(cx, variant.hir_id, variant.ident.span, "enum variant")
        else {
            return;
        };
        violation.emit(cx);
    }

    fn check_trait_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx TraitItem<'tcx>) {
        // Trait items outside a module-level trait are outside this policy.
        if !Self::is_module_level(cx, item.hir_id()) {
            return;
        }

        // Documented or generated trait items produce no violation.
        let Some(violation) =
            Violation::from_declaration(cx, item.hir_id(), item.ident.span, "trait item")
        else {
            return;
        };
        violation.emit(cx);
    }

    fn check_impl_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx ImplItem<'tcx>) {
        // Nested items and trait-provided contracts do not need inherent-item documentation here.
        if !Self::is_module_level(cx, item.hir_id()) || Self::is_trait_implementation_item(cx, item)
        {
            return;
        }

        // Documented or generated inherent items produce no violation.
        let Some(violation) =
            Violation::from_declaration(cx, item.hir_id(), item.ident.span, "inherent impl item")
        else {
            return;
        };
        violation.emit(cx);
    }
}
