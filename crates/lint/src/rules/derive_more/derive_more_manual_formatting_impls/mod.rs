extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use rustc_errors::DiagDecorator;
use rustc_hir::def::{CtorOf, DefKind, Res};
use rustc_hir::{
    Expr, ExprKind, ImplItem, ImplItemKind, ItemKind, Mutability, Node, PatExprKind, PatKind,
};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty;
use rustc_span::Span;
use rustc_span::def_id::{DefId, LocalDefId};
use syn::parse::Parser;
use syn::punctuated::Punctuated;

use crate::config::providers::DisplayProvider;
use crate::config::store::ConfigStore;
use crate::utils::diagnostic::LateViolation;
use crate::utils::direct_forwarding::DirectForwarding;

// -----------------------------------------------------------------------------
// Violation: Derivable formatting implementation family
// -----------------------------------------------------------------------------

/// Formatting implementations for one wrapper that share a transparent field policy.
struct Family {
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Local type name used to identify the affected derive contract.
    name: String,
    /// Formatting traits whose implementations share the same transparent field policy.
    traits: Vec<&'static str>,
}

/// Complete formatting family proven replaceable by `derive_more`.
struct Violation(
    /// Formatting family that triggered the violation.
    Family,
);

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "manual formatting for `{}` is derivable",
            self.0.name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "each implementation is one exact field delegation or one format expression without policy-bearing control flow",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        let derives = self
            .0
            .traits
            .iter()
            .map(|name| format!("derive_more::{name}"))
            .collect::<Vec<_>>()
            .join(", ");

        Cow::Owned(format!(
            "replace this family with `#[derive({derives})]`, preserving exact format strings in derive_more attributes"
        ))
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            DERIVE_MORE_MANUAL_FORMATTING_IMPLS,
            self.0.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.0.span, "this type owns derivable formatting plumbing");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// DeriveMoreManualFormattingImpls: Declarative formatting policy
// -----------------------------------------------------------------------------

/// Groups transparent formatting implementations by their wrapper type.
struct DeriveMoreManualFormattingImpls {
    /// Formatting families accumulated until all implementations have been visited.
    families: HashMap<LocalDefId, Family>,
    /// Explicit framework selected for enum display generation.
    display_provider: Option<DisplayProvider>,
}

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub DERIVE_MORE_MANUAL_FORMATTING_IMPLS,
    Warn,
    "finds formatting implementations reproducible by derive_more",
    DeriveMoreManualFormattingImpls::new()
}

impl DeriveMoreManualFormattingImpls {
    /// Starts formatting analysis with project provider policy.
    fn new() -> Self {
        Self {
            families: HashMap::new(),
            display_provider: ConfigStore::get().derive_resolution.enum_display(),
        }
    }

    /// Maps a standard formatting trait to its `derive_more` macro name.
    fn formatting_trait(name: &str) -> Option<&'static str> {
        match name {
            "Debug" => Some("Debug"),
            "Display" => Some("Display"),
            "Binary" => Some("Binary"),
            "Octal" => Some("Octal"),
            "LowerHex" => Some("LowerHex"),
            "UpperHex" => Some("UpperHex"),
            "LowerExp" => Some("LowerExp"),
            "UpperExp" => Some("UpperExp"),
            "Pointer" => Some("Pointer"),
            _ => None,
        }
    }

    /// Returns whether an expression borrows a field of the receiver binding.
    fn is_field_reference(
        cx: &LateContext<'_>,
        expression: &Expr<'_>,
        binding: rustc_hir::HirId,
    ) -> bool {
        let expression = match expression.kind {
            ExprKind::AddrOf(_, Mutability::Not, inner) => inner,
            _ => expression,
        };

        // Direct formatting delegation must select a receiver field.
        let ExprKind::Field(base, _) = expression.kind else {
            return false;
        };
        DirectForwarding::is_binding(cx, base, binding)
    }

    /// Recognizes formatting that delegates the same trait directly to one field.
    fn is_direct_trait_delegation(
        cx: &LateContext<'_>,
        owner: LocalDefId,
        expression: &Expr<'_>,
        self_binding: rustc_hir::HirId,
        formatter_binding: rustc_hir::HirId,
        trait_id: DefId,
    ) -> bool {
        // Exact delegation requires a resolved direct call.
        let Some(call) = DirectForwarding::call(cx, owner, expression) else {
            return false;
        };

        // Formatting traits receive the formatted value and formatter arguments.
        let [value, formatter] = call.arguments.as_slice() else {
            return false;
        };
        cx.tcx.trait_of_assoc(call.target) == Some(trait_id)
            && Self::is_field_reference(cx, value, self_binding)
            && DirectForwarding::is_binding(cx, formatter, formatter_binding)
    }

    /// Counts unescaped placeholders in one format string.
    fn placeholder_count(format: &str) -> usize {
        let mut placeholders = 0;
        let mut characters = format.chars().peekable();
        while let Some(character) = characters.next() {
            if character != '{' {
                continue;
            }
            if characters.peek() == Some(&'{') {
                characters.next();
                continue;
            }
            placeholders += 1;
        }
        placeholders
    }

    /// Returns whether `derive_more` can reproduce an authored receiver projection.
    fn is_receiver_projection(expression: &syn::Expr) -> bool {
        match expression {
            syn::Expr::Field(field) => {
                matches!(
                    field.base.as_ref(),
                    syn::Expr::Path(base) if base.path.is_ident("self")
                ) || Self::is_receiver_projection(field.base.as_ref())
            }
            syn::Expr::MethodCall(call) => {
                call.args.is_empty() && Self::is_receiver_projection(call.receiver.as_ref())
            }
            syn::Expr::Paren(expression) => Self::is_receiver_projection(&expression.expr),
            syn::Expr::Reference(expression) => Self::is_receiver_projection(&expression.expr),
            _ => false,
        }
    }

    /// Recognizes one `write!` invocation over receiver-owned values.
    fn is_single_write(cx: &LateContext<'_>, item: &ImplItem<'_>, expression: &Expr<'_>) -> bool {
        let is_standard_write = expression.span.macro_backtrace().any(|expansion| {
            expansion.macro_def_id.is_some_and(|definition| {
                cx.tcx.item_name(definition).as_str() == "write"
                    && matches!(cx.tcx.crate_name(definition.krate).as_str(), "core" | "std")
            })
        });

        // Only the standard `write!` macro has the formatting semantics recognized here.
        if !is_standard_write {
            return false;
        }

        // The authored implementation source must be available for syntax inspection.
        let Ok(source) = cx.sess().source_map().span_to_snippet(item.span) else {
            return false;
        };

        // The implementation source must parse as a method item.
        let Ok(method) = syn::parse_str::<syn::ImplItemFn>(&source) else {
            return false;
        };

        // The method body must contain exactly one macro expression statement.
        let [syn::Stmt::Expr(syn::Expr::Macro(invocation), _)] = method.block.stmts.as_slice()
        else {
            return false;
        };

        // The sole body expression must invoke the standard `write!` macro.
        if invocation
            .mac
            .path
            .segments
            .last()
            .is_none_or(|segment| segment.ident != "write")
        {
            return false;
        }
        let inputs = method.sig.inputs.iter().collect::<Vec<_>>();

        // Standard formatting methods take the receiver followed by one formatter parameter.
        let [
            syn::FnArg::Receiver(_),
            syn::FnArg::Typed(formatter_parameter),
        ] = inputs.as_slice()
        else {
            return false;
        };

        // The formatter parameter must be an identifier for macro-argument matching.
        let syn::Pat::Ident(formatter_parameter) = formatter_parameter.pat.as_ref() else {
            return false;
        };
        let parser = Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated;

        // Macro arguments must parse as comma-separated Rust expressions.
        let Ok(arguments) = parser.parse2(invocation.mac.tokens.clone()) else {
            return false;
        };

        let arguments = arguments.iter().collect::<Vec<_>>();

        // Exact formatting requires the formatter, a literal, and receiver-owned values.
        let [formatter, syn::Expr::Lit(format), fields @ ..] = arguments.as_slice() else {
            return false;
        };

        // The first macro argument must be the formatter identifier.
        let syn::Expr::Path(formatter) = formatter else {
            return false;
        };

        // The format argument must be a string literal.
        let syn::Lit::Str(format) = &format.lit else {
            return false;
        };

        // A derive attribute needs at least one value and only receiver-owned projections.
        if fields.is_empty()
            || !fields
                .iter()
                .all(|field| Self::is_receiver_projection(field))
        {
            return false;
        }

        formatter.path.is_ident(&formatter_parameter.ident)
            && Self::placeholder_count(&format.value()) == fields.len()
    }

    /// Recognizes `formatter.write_str(self.field)` as transparent field formatting.
    fn is_direct_write_str(
        cx: &LateContext<'_>,
        owner: LocalDefId,
        expression: &Expr<'_>,
        self_binding: rustc_hir::HirId,
        formatter_binding: rustc_hir::HirId,
    ) -> bool {
        // Only a resolved direct call can prove the standard formatter contract.
        let Some(call) = DirectForwarding::call(cx, owner, expression) else {
            return false;
        };

        // `write_str` receives exactly the formatter and borrowed string value.
        let [formatter, value] = call.arguments.as_slice() else {
            return false;
        };
        cx.tcx.item_name(call.target).as_str() == "write_str"
            && cx.tcx.crate_name(call.target.krate).as_str() == "core"
            && DirectForwarding::is_binding(cx, formatter, formatter_binding)
            && Self::is_field_reference(cx, value, self_binding)
    }

    /// Resolves one ignored enum variant pattern.
    fn unit_variant(cx: &LateContext<'_>, pattern: &rustc_hir::Pat<'_>) -> Option<LocalDefId> {
        // Unit variants lower to expression patterns rather than binding patterns.
        let PatKind::Expr(expression) = pattern.kind else {
            return None;
        };

        // Only a resolved path can identify the variant declaration.
        let PatExprKind::Path(path) = expression.kind else {
            return None;
        };
        match cx.qpath_res(&path, expression.hir_id) {
            Res::Def(DefKind::Variant, variant) => variant.as_local(),
            Res::Def(DefKind::Ctor(CtorOf::Variant, _), constructor) => {
                cx.tcx.opt_local_parent(constructor.as_local()?)
            }
            _ => None,
        }
    }

    /// Recognizes one exhaustive enum match passed to `Formatter::write_str`.
    fn is_enum_write_str_match(
        cx: &LateContext<'_>,
        owner: LocalDefId,
        expression: &Expr<'_>,
        definition: LocalDefId,
        self_binding: rustc_hir::HirId,
        formatter_binding: rustc_hir::HirId,
    ) -> bool {
        // Enum display recognition begins at the resolved `write_str` call.
        let Some(call) = DirectForwarding::call(cx, owner, expression) else {
            return false;
        };

        // `write_str` receives exactly the formatter and selected string value.
        let [formatter, value] = call.arguments.as_slice() else {
            return false;
        };

        // Other calls or formatter values do not establish the standard display contract.
        if cx.tcx.item_name(call.target).as_str() != "write_str"
            || cx.tcx.crate_name(call.target.krate).as_str() != "core"
            || !DirectForwarding::is_binding(cx, formatter, formatter_binding)
        {
            return false;
        }

        // Static enum display metadata must come from an explicit exhaustive match.
        let ExprKind::Match(scrutinee, arms, _) = value.kind else {
            return false;
        };

        // Matching a derived or unrelated value would encode additional behavior.
        if !DirectForwarding::is_binding(cx, scrutinee, self_binding) {
            return false;
        }
        let mut observed = HashSet::new();
        for arm in arms {
            // Every arm must be one unguarded unit variant.
            let Some(variant) = (arm.guard.is_none())
                .then(|| Self::unit_variant(cx, arm.pat))
                .flatten()
            else {
                return false;
            };

            // Derive metadata can reproduce only literal string results.
            let ExprKind::Lit(literal) = arm.body.kind else {
                return false;
            };

            // Foreign, repeated, or non-string arms do not form an exact local mapping.
            if !matches!(literal.node, rustc_ast::LitKind::Str(..))
                || cx.tcx.opt_local_parent(variant) != Some(definition)
                || !observed.insert(variant)
            {
                return false;
            }
        }
        let variants = cx.tcx.adt_def(definition.to_def_id()).variants();
        variants.len() == observed.len()
            && variants.iter().all(|variant| {
                variant
                    .def_id
                    .as_local()
                    .is_some_and(|id| observed.contains(&id))
            })
    }
}

impl<'tcx> LateLintPass<'tcx> for DeriveMoreManualFormattingImpls {
    fn check_impl_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx ImplItem<'tcx>) {
        // Only exact formatting implementations contribute to a derive family.
        let Some(ExactFormatting {
            definition,
            trait_name,
        }) = ExactFormatting::analyze(cx, item, self.display_provider)
        else {
            return;
        };
        let family = self.families.entry(definition).or_insert_with(|| Family {
            span: cx.tcx.def_span(definition),
            name: cx.tcx.item_name(definition).to_string(),
            traits: Vec::new(),
        });

        // A trait appears only once in the derive set for its wrapper.
        if family.traits.contains(&trait_name) {
            return;
        }
        family.traits.push(trait_name);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        let mut families = self
            .families
            .drain()
            .map(|(_, family)| family)
            .collect::<Vec<_>>();
        families.sort_by_key(|family| family.span.lo());
        for mut family in families {
            family.traits.sort_unstable();
            Violation(family).emit(cx);
        }
    }
}

/// Identifies one exact formatting implementation.
struct ExactFormatting {
    /// Local type receiving the formatting implementation.
    definition: LocalDefId,
    /// Formatting trait implemented by the analyzed method.
    trait_name: &'static str,
}

impl ExactFormatting {
    /// Recognizes one exact formatting implementation.
    fn analyze(
        cx: &LateContext<'_>,
        item: &ImplItem<'_>,
        display_provider: Option<DisplayProvider>,
    ) -> Option<Self> {
        // Formatting analysis applies only to implementation methods.
        let ImplItemKind::Fn(signature, body_id) = item.kind else {
            return None;
        };

        // Only authored `fmt` methods can implement a formatting trait.
        if item.ident.name.as_str() != "fmt" || item.span.from_expansion() {
            return None;
        }
        let implementation = cx.tcx.local_parent(item.owner_id.def_id);

        // The method must be enclosed by an implementation item.
        let Node::Item(parent) = cx.tcx.hir_node_by_def_id(implementation) else {
            return None;
        };

        // That parent must remain an implementation after HIR resolution.
        let ItemKind::Impl(implementation_item) = parent.kind else {
            return None;
        };

        // Behavioral attributes preserve authored formatting; rustdoc remains nonsemantic.
        if !DirectForwarding::has_only_nonsemantic_attributes(cx, parent.hir_id())
            || !DirectForwarding::has_only_nonsemantic_attributes(cx, item.hir_id())
        {
            return None;
        }
        let trait_id = implementation_item.of_trait?.trait_ref.trait_def_id()?;

        // Only core formatting traits can be reproduced by derive_more formatting derives.
        if cx.tcx.crate_name(trait_id.krate).as_str() != "core" {
            return None;
        }
        let trait_name =
            DeriveMoreManualFormattingImpls::formatting_trait(cx.tcx.item_name(trait_id).as_str())?;
        let trait_ref = cx.tcx.impl_trait_ref(implementation).instantiate_identity();

        // The formatted implementation target must be a nominal type.
        let ty::Adt(definition, _) = trait_ref.self_ty().kind() else {
            return None;
        };

        // Unions cannot receive any formatting derive contract recognized here.
        if !definition.is_struct() && !definition.is_enum() {
            return None;
        }
        let definition = definition.did().as_local()?;

        let body = cx.tcx.hir_body(body_id);
        let forwarding =
            DirectForwarding::expression(cx, item.owner_id.def_id, signature.header, body)?;

        // `fmt` forwarding requires the receiver and formatter bindings in order.
        let [self_binding, formatter_binding] = forwarding.bindings.as_slice() else {
            return None;
        };

        let is_enum = cx.tcx.adt_def(definition.to_def_id()).is_enum();
        let exact = if is_enum {
            display_provider == Some(DisplayProvider::DeriveMoreDisplay)
                && trait_name == "Display"
                && DeriveMoreManualFormattingImpls::is_enum_write_str_match(
                    cx,
                    forwarding.typeck_owner,
                    forwarding.forwarded,
                    definition,
                    *self_binding,
                    *formatter_binding,
                )
        } else {
            DeriveMoreManualFormattingImpls::is_direct_trait_delegation(
                cx,
                forwarding.typeck_owner,
                forwarding.forwarded,
                *self_binding,
                *formatter_binding,
                trait_id,
            ) || DeriveMoreManualFormattingImpls::is_direct_write_str(
                cx,
                forwarding.typeck_owner,
                forwarding.forwarded,
                *self_binding,
                *formatter_binding,
            ) || DeriveMoreManualFormattingImpls::is_single_write(cx, item, forwarding.forwarded)
        };

        if exact {
            Some(Self {
                definition,
                trait_name,
            })
        } else {
            None
        }
    }
}
