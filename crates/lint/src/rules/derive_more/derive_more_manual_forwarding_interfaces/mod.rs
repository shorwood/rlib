extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::HashMap;

use rustc_errors::DiagDecorator;
use rustc_hir::{Expr, ExprKind, ImplItem, ImplItemKind, ItemKind, Mutability, Node};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty;
use rustc_span::Span;
use rustc_span::def_id::{DefId, LocalDefId};

use crate::utils::diagnostic::LateViolation;
use crate::utils::direct_forwarding::DirectForwarding;

// -----------------------------------------------------------------------------
// Violation: Derivable forwarding interface
// -----------------------------------------------------------------------------

/// One `derive_more` forwarding contract proven by an authored implementation.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Contract {
    /// `derive_more` macro capable of replacing the implementation.
    derive: &'static str,
    /// Whether the implementation delegates behavior rather than exposing a field directly.
    is_direct_delegation: bool,
}

/// Standard interfaces exposing the same wrapper field without additional policy.
struct Family {
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Local type name used to identify the affected derive contract.
    name: String,
    /// Forwarding interfaces implemented by the same wrapper.
    contracts: Vec<Contract>,
}

/// Complete forwarding family proven replaceable by `derive_more`.
struct Violation(
    /// Forwarding-interface family that triggered the violation.
    Family,
);

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "manual forwarding interfaces for `{}` are derivable",
            self.0.name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "these implementations only expose one field directly or delegate to that field's matching standard interface",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        let derives = self
            .0
            .contracts
            .iter()
            .map(|contract| format!("derive_more::{}", contract.derive))
            .collect::<Vec<_>>()
            .join(", ");

        let forwarding = self
            .0
            .contracts
            .iter()
            .any(|contract| contract.is_direct_delegation);

        let qualifier = if forwarding {
            "; preserve pass-through targets with the corresponding `forward` attribute"
        } else {
            ""
        };

        Cow::Owned(format!(
            "replace this family with `#[derive({derives})]`{qualifier}"
        ))
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            DERIVE_MORE_MANUAL_FORWARDING_INTERFACES,
            self.0.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.0.span, "this type owns structural forwarding plumbing");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

/// Describes a borrowed field expression and its receiver.
struct DirectFieldReference<'hir> {
    /// Expression from which the field is projected.
    base: &'hir Expr<'hir>,
    /// Complete field projection expression.
    field: &'hir Expr<'hir>,
    /// Mutability of the borrow.
    mutability: Mutability,
}

impl<'hir> DirectFieldReference<'hir> {
    /// Recognizes a direct borrow of one field.
    const fn from_expr(expression: &'hir Expr<'hir>) -> Option<Self> {
        // Direct field forwarding begins with an explicit borrow expression.
        let ExprKind::AddrOf(_, mutability, field_expression) = expression.kind else {
            return None;
        };

        // The borrowed expression must be a field projection.
        let ExprKind::Field(base, _) = field_expression.kind else {
            return None;
        };
        Some(Self {
            base,
            field: field_expression,
            mutability,
        })
    }
}

/// Receiver and index arguments required by an indexing operation.
const INDEX_ARGUMENT_COUNT: usize = 2;

/// Receiver and index bindings required by an indexing implementation.
const INDEX_BINDING_COUNT: usize = 2;

// -----------------------------------------------------------------------------
// DeriveMoreManualForwardingInterfaces: Declarative forwarding policy
// -----------------------------------------------------------------------------

/// Validated indexing derive name passed to direct syntax analysis.
struct IndexDerive(
    /// Validated `Index` or `IndexMut` derive spelling.
    &'static str,
);

/// Groups transparent forwarding implementations by their wrapper type.
#[derive(Default)]
struct DeriveMoreManualForwardingInterfaces {
    /// Forwarding families accumulated until every implementation is known.
    families: HashMap<LocalDefId, Family>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub DERIVE_MORE_MANUAL_FORWARDING_INTERFACES,
    Warn,
    "finds forwarding interfaces reproducible by derive_more",
    DeriveMoreManualForwardingInterfaces::default()
}

impl DeriveMoreManualForwardingInterfaces {
    /// Resolves a supported standard forwarding trait to its derive name.
    fn supported_trait(
        cx: &LateContext<'_>,
        trait_id: DefId,
        method: &str,
    ) -> Option<&'static str> {
        // Only core forwarding traits have matching derive_more implementations.
        if cx.tcx.crate_name(trait_id.krate).as_str() != "core" {
            return None;
        }

        match (cx.tcx.item_name(trait_id).as_str(), method) {
            ("AsRef", "as_ref") => Some("AsRef"),
            ("AsMut", "as_mut") => Some("AsMut"),
            ("Deref", "deref") => Some("Deref"),
            ("DerefMut", "deref_mut") => Some("DerefMut"),
            ("Index", "index") => Some("Index"),
            ("IndexMut", "index_mut") => Some("IndexMut"),
            _ => None,
        }
    }

    /// Returns the borrow mutability required by a forwarding derive.
    fn expected_mutability(derive: &str) -> Mutability {
        if matches!(derive, "AsMut" | "DerefMut" | "IndexMut") {
            Mutability::Mut
        } else {
            Mutability::Not
        }
    }

    /// Returns whether an argument is the expected receiver field with matching mutability.
    fn field_argument(
        cx: &LateContext<'_>,
        expression: &Expr<'_>,
        binding: rustc_hir::HirId,
        mutability: Mutability,
    ) -> bool {
        let expression = match expression.kind {
            ExprKind::AddrOf(_, actual, inner) if actual == mutability => inner,
            _ => expression,
        };

        // The forwarded receiver must be selected through a field projection.
        let ExprKind::Field(base, _) = expression.kind else {
            return false;
        };
        DirectForwarding::is_binding(cx, base, binding)
    }

    /// Recognizes ordinary indexing syntax over one receiver field.
    fn direct_index(
        cx: &LateContext<'_>,
        owner: LocalDefId,
        expression: &Expr<'_>,
        bindings: &[rustc_hir::HirId],
        trait_id: DefId,
        derive: &IndexDerive,
    ) -> bool {
        // Direct indexing forwarding begins with a borrowed indexing expression.
        let ExprKind::AddrOf(_, mutability, indexed) = expression.kind else {
            return false;
        };

        // The borrow mutability must match the selected indexing derive.
        if mutability != Self::expected_mutability(derive.0) {
            return false;
        }

        // The borrowed expression must use ordinary indexing syntax.
        let ExprKind::Index(container, index, _) = indexed.kind else {
            return false;
        };

        // The index container must be the wrapper's stored field.
        let ExprKind::Field(base, _) = container.kind else {
            return false;
        };
        let target = cx.tcx.typeck(owner).type_dependent_def_id(indexed.hir_id);
        target.is_some_and(|target| cx.tcx.trait_of_assoc(target) == Some(trait_id))
            && DirectForwarding::is_binding(cx, base, bindings[0])
            && DirectForwarding::is_binding(cx, index, bindings[1])
    }
}

impl<'tcx> LateLintPass<'tcx> for DeriveMoreManualForwardingInterfaces {
    fn check_impl_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx ImplItem<'tcx>) {
        // Only exact forwarding methods contribute a derive contract to the family.
        let Some(ContractTarget {
            definition,
            contract,
        }) = ContractTarget::for_item(cx, item)
        else {
            return;
        };
        let family = self.families.entry(definition).or_insert_with(|| Family {
            span: cx.tcx.def_span(definition),
            name: cx.tcx.item_name(definition).to_string(),
            contracts: Vec::new(),
        });

        // Each trait is represented once in the wrapper's derive set.
        if family.contracts.contains(&contract) {
            return;
        }
        family.contracts.push(contract);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        let mut families = self
            .families
            .drain()
            .map(|(_, family)| family)
            .collect::<Vec<_>>();
        families.sort_by_key(|family| family.span.lo());
        for mut family in families {
            family.contracts.sort();
            Violation(family).emit(cx);
        }
    }
}

/// Connects a forwarding contract to the local wrapper that implements it.
struct ContractTarget {
    /// Local wrapper type receiving the implementation.
    definition: LocalDefId,
    /// Forwarding behavior implemented for the wrapper.
    contract: Contract,
}

impl ContractTarget {
    /// Recognizes a method that forwards through the wrapped field's trait.
    fn from_forwarded_call(
        cx: &LateContext<'_>,
        owner: LocalDefId,
        expression: &Expr<'_>,
        bindings: &[rustc_hir::HirId],
        trait_id: DefId,
        derive: &'static str,
        definition: LocalDefId,
    ) -> Option<Self> {
        let call = DirectForwarding::call(cx, owner, expression)?;

        // The forwarded call must resolve to the implemented standard trait method.
        if cx.tcx.trait_of_assoc(call.target) != Some(trait_id) {
            return None;
        }
        let first = call.arguments.first()?;

        // The first call argument must be the expected wrapper field borrow.
        if !DeriveMoreManualForwardingInterfaces::field_argument(
            cx,
            first,
            bindings[0],
            DeriveMoreManualForwardingInterfaces::expected_mutability(derive),
        ) {
            return None;
        }

        // Indexing calls must preserve exactly one index binding after the receiver.
        if matches!(derive, "Index" | "IndexMut")
            && (call.arguments.len() != INDEX_ARGUMENT_COUNT
                || !DirectForwarding::is_binding(cx, call.arguments[1], bindings[1]))
        {
            return None;
        }

        Some(Self {
            definition,
            contract: Contract {
                derive,
                is_direct_delegation: true,
            },
        })
    }

    /// Recognizes one exact field forwarding contract.
    fn for_item(cx: &LateContext<'_>, item: &ImplItem<'_>) -> Option<Self> {
        // Forwarding analysis applies only to implementation methods.
        let ImplItemKind::Fn(signature, body_id) = item.kind else {
            return None;
        };

        // Macro-expanded methods are not reliable authored forwarding contracts.
        if item.span.from_expansion() {
            return None;
        }
        let implementation = cx.tcx.local_parent(item.owner_id.def_id);

        // The method must be enclosed by an implementation item.
        let Node::Item(parent) = cx.tcx.hir_node_by_def_id(implementation) else {
            return None;
        };

        // The enclosing item must remain an implementation after HIR resolution.
        let ItemKind::Impl(implementation_item) = parent.kind else {
            return None;
        };

        // Attributes may carry behavior beyond transparent field forwarding.
        if !cx.tcx.hir_attrs(parent.hir_id()).is_empty()
            || !cx.tcx.hir_attrs(item.hir_id()).is_empty()
        {
            return None;
        }
        let trait_id = implementation_item.of_trait?.trait_ref.trait_def_id()?;

        let derive = DeriveMoreManualForwardingInterfaces::supported_trait(
            cx,
            trait_id,
            item.ident.name.as_str(),
        )?;
        let trait_ref = cx.tcx.impl_trait_ref(implementation).instantiate_identity();

        // The implementation target must be a nominal wrapper type.
        let ty::Adt(wrapper, _) = trait_ref.self_ty().kind() else {
            return None;
        };
        let definition = wrapper.did().as_local()?;

        // This derive recognizer supports struct wrappers only.
        if !wrapper.is_struct() {
            return None;
        }
        let body = cx.tcx.hir_body(body_id);
        let forwarding =
            DirectForwarding::expression(cx, item.owner_id.def_id, signature.header, body)?;

        let expected_bindings = if matches!(derive, "Index" | "IndexMut") {
            INDEX_BINDING_COUNT
        } else {
            1
        };

        // The forwarding body must bind exactly the receiver and required index arguments.
        if forwarding.bindings.len() != expected_bindings {
            return None;
        }
        let self_binding = forwarding.bindings[0];

        // Non-indexing derives can expose the field reference directly.
        if !matches!(derive, "Index" | "IndexMut")
            && let Some(field) = DirectFieldReference::from_expr(forwarding.forwarded)
            && field.mutability == DeriveMoreManualForwardingInterfaces::expected_mutability(derive)
            && DirectForwarding::is_binding(cx, field.base, self_binding)
        {
            let field_type = cx.tcx.typeck(forwarding.typeck_owner).expr_ty(field.field);
            let output = cx
                .tcx
                .fn_sig(item.owner_id.def_id)
                .instantiate_identity()
                .skip_binder()
                .output();

            // Direct field accessors must return a reference to the accessed field type.
            let ty::Ref(_, target, _) = output.kind() else {
                return None;
            };

            return Some(Self {
                definition,
                contract: Contract {
                    derive,
                    is_direct_delegation: field_type != *target,
                },
            });
        }

        // Indexing derives can preserve ordinary indexing syntax directly.
        if matches!(derive, "Index" | "IndexMut")
            && DeriveMoreManualForwardingInterfaces::direct_index(
                cx,
                forwarding.typeck_owner,
                forwarding.forwarded,
                &forwarding.bindings,
                trait_id,
                &IndexDerive(derive),
            )
        {
            return Some(Self {
                definition,
                contract: Contract {
                    derive,
                    is_direct_delegation: true,
                },
            });
        }

        Self::from_forwarded_call(
            cx,
            forwarding.typeck_owner,
            forwarding.forwarded,
            &forwarding.bindings,
            trait_id,
            derive,
            definition,
        )
    }
}
