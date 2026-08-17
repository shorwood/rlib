extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::HashSet;

use rstml::node::{Node, NodeAttribute};
use rustc_errors::DiagDecorator;
use rustc_hir::Expr;
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::hygiene::{ExpnKind, MacroKind};
use rustc_span::{BytePos, Span};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{ExprMacro, Pat};

use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Key: Authored unstable-key evidence
// -----------------------------------------------------------------------------

/// Absolute source coordinates for one reported key expression.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct KeySourceRange {
    /// Inclusive low byte position.
    lo: u32,
    /// Exclusive high byte position.
    hi: u32,
}

/// Locates one unstable key expression in authored view source.
struct KeyFinding {
    /// Byte offset of the key expression.
    start: usize,
    /// Byte length of the unstable expression prefix.
    length: usize,
    /// Reason the expression cannot preserve item identity.
    reason: &'static str,
}

// -----------------------------------------------------------------------------
// ParameterUse: Key closure provenance
// -----------------------------------------------------------------------------

/// Collects closure parameter names used by a key expression.
struct ParameterUse<'names> {
    /// Closure bindings that could establish row-derived identity.
    names: &'names HashSet<String>,
    /// Whether the key body refers to any such binding.
    is_used: bool,
}

impl<'ast> Visit<'ast> for ParameterUse<'_> {
    fn visit_expr_path(&mut self, path: &'ast syn::ExprPath) {
        if path.qself.is_none()
            && path.path.segments.len() == 1
            && self
                .names
                .contains(&path.path.segments[0].ident.to_string())
        {
            self.is_used = true;
        }
        visit::visit_expr_path(self, path);
    }
}

// -----------------------------------------------------------------------------
// PatternBindings: Closure parameter names
// -----------------------------------------------------------------------------

/// Collects identifiers introduced by a closure parameter pattern.
#[derive(Default)]
struct PatternBindings {
    /// Authored binding names.
    names: HashSet<String>,
}

impl<'ast> Visit<'ast> for PatternBindings {
    fn visit_pat_ident(&mut self, pattern: &'ast syn::PatIdent) {
        self.names.insert(pattern.ident.to_string());
        visit::visit_pat_ident(self, pattern);
    }
}

// -----------------------------------------------------------------------------
// EnumerationUse: Positional traversal evidence
// -----------------------------------------------------------------------------

/// Finds whether an `each` expression uses positional enumeration.
#[derive(Default)]
struct EnumerationUse {
    /// A standard-looking enumerate adapter occurs in the expression.
    is_found: bool,
}

impl<'ast> Visit<'ast> for EnumerationUse {
    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        self.is_found |= call.method == "enumerate" && call.args.is_empty();
        visit::visit_expr_method_call(self, call);
    }
}

// -----------------------------------------------------------------------------
// CollectionIdentity: Traversal identity policy
// -----------------------------------------------------------------------------

/// Whether collection traversal exposes positional rather than stable identity.
#[derive(Clone, Copy)]
enum CollectionIdentity {
    /// Items retain domain-derived identity.
    Stable,
    /// Items are paired with traversal positions.
    Positional,
}

impl CollectionIdentity {
    /// Classifies whether traversal exposes positions as item identity.
    fn from_traversal(terminal: &str, enumeration: &EnumerationUse) -> Self {
        if terminal == "ForEnumerate" || enumeration.is_found {
            Self::Positional
        } else {
            Self::Stable
        }
    }

    /// Returns whether traversal identity is derived from an item position.
    const fn is_positional(self) -> bool {
        matches!(self, Self::Positional)
    }
}

// -----------------------------------------------------------------------------
// Violation: Unstable key diagnostic
// -----------------------------------------------------------------------------

/// Key expression that cannot retain stable item identity.
struct Violation {
    /// Function body used to honor local lint attributes.
    owner: rustc_hir::HirId,
    /// Authored key expression highlighted by the diagnostic.
    span: Span,
    /// Concise reason the key is unstable.
    reason: &'static str,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("`<For>` key does not represent stable item identity")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "keys preserve the relationship between data and rendered rows when a collection changes",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "use a unique domain identifier that remains stable across inserts and reordering",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            LEPTOS_UNSTABLE_FOR_KEYS,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, self.reason);
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// LeptosUnstableForKeys: Key identity policy
// -----------------------------------------------------------------------------
/// Late lint pass that rejects constant and position-based keyed iteration.
#[derive(Default)]
struct LeptosUnstableForKeys {
    /// Absolute source ranges already diagnosed while visiting one expanded view.
    reported: HashSet<KeySourceRange>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_UNSTABLE_FOR_KEYS,
    Warn,
    "rejects constant and position-based Leptos For keys",
    LeptosUnstableForKeys::default()
}

impl LeptosUnstableForKeys {
    /// Returns the single binding returned directly by a key body, if any.
    fn returned_binding(expression: &syn::Expr) -> Option<String> {
        let expression = match expression {
            syn::Expr::Paren(paren) => &*paren.expr,
            syn::Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Deref(_)) => &*unary.expr,
            _ => expression,
        };
        let syn::Expr::Path(path) = expression else {
            return None;
        };
        (path.qself.is_none() && path.path.segments.len() == 1)
            .then(|| path.path.segments[0].ident.to_string())
    }

    /// Classifies parsed key closure semantics against its collection source.
    fn key_reason(expression: &syn::Expr, identity: CollectionIdentity) -> Option<&'static str> {
        let syn::Expr::Closure(closure) = expression else {
            return None;
        };
        let mut bindings = PatternBindings::default();
        for input in &closure.inputs {
            bindings.visit_pat(input);
        }
        let output = match &*closure.body {
            syn::Expr::Block(block) => {
                block
                    .block
                    .stmts
                    .last()
                    .and_then(|statement| match statement {
                        syn::Stmt::Expr(expression, _) => Some(expression),
                        _ => None,
                    })
            }
            expression => Some(expression),
        };
        if output.is_some_and(|expression| {
            matches!(expression, syn::Expr::Lit(_))
                || matches!(expression, syn::Expr::Tuple(tuple) if tuple.elems.is_empty())
        }) {
            return Some("the key does not derive from row identity");
        }
        let mut usage = ParameterUse {
            names: &bindings.names,
            is_used: false,
        };
        usage.visit_expr(&closure.body);
        if !usage.is_used {
            return Some("the key does not derive from row identity");
        }

        let returned = Self::returned_binding(&closure.body)?;
        let positional_binding = closure.inputs.first().and_then(|input| match input {
            Pat::Ident(binding) => Some(binding.ident.to_string()),
            Pat::Tuple(tuple) => tuple.elems.first().and_then(|element| match element {
                Pat::Ident(binding) => Some(binding.ident.to_string()),
                _ => None,
            }),
            _ => None,
        });
        (identity.is_positional() && positional_binding.as_deref() == Some(&returned))
            .then_some("the row position changes when the collection is reordered")
    }

    /// Finds unstable key closures inside parsed authored `<For>` markup.
    fn findings(source: &str) -> Vec<KeyFinding> {
        let Ok(expression) = syn::parse_str::<ExprMacro>(source) else {
            return Vec::new();
        };
        let parser = rstml::Parser::new(rstml::ParserConfig::default().recover_block(true));
        let (nodes, errors) = parser.parse_recoverable(expression.mac.tokens).split_vec();
        if !errors.is_empty() {
            return Vec::new();
        }

        nodes
            .into_iter()
            .flat_map(Node::flatten)
            .filter_map(|node| {
                let Node::Element(element) = node else {
                    return None;
                };
                let name = element.name().to_string();
                let terminal = name.rsplit("::").next().unwrap_or(&name);
                if !matches!(terminal, "For" | "ForEnumerate") {
                    return None;
                }
                let attribute = |name: &str| {
                    element.attributes().iter().find_map(|attribute| {
                        let NodeAttribute::Attribute(attribute) = attribute else {
                            return None;
                        };
                        (attribute.key.to_string() == name)
                            .then(|| attribute.value())
                            .flatten()
                    })
                };
                let key = attribute("key")?;
                let mut enumeration = EnumerationUse::default();
                if let Some(each) = attribute("each") {
                    enumeration.visit_expr(each);
                }
                let identity = CollectionIdentity::from_traversal(terminal, &enumeration);
                let reason = Self::key_reason(key, identity)?;
                let range = key.span().byte_range();
                Some(KeyFinding {
                    start: range.start,
                    length: range.end.saturating_sub(range.start),
                    reason,
                })
            })
            .collect()
    }
}

impl<'tcx> LateLintPass<'tcx> for LeptosUnstableForKeys {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) {
        let Some(view_span) = expression.span.macro_backtrace().find_map(|expansion| {
            matches!(expansion.kind, ExpnKind::Macro(MacroKind::Bang, name) if name.as_str() == "view")
                .then_some(expansion.call_site)
        }) else {
            return;
        };
        let Ok(source) = cx.sess().source_map().span_to_snippet(view_span) else {
            return;
        };
        for KeyFinding {
            start: offset,
            length,
            reason,
        } in Self::findings(&source)
        {
            let (Ok(offset), Ok(length)) = (u32::try_from(offset), u32::try_from(length)) else {
                continue;
            };
            let lo = view_span.lo() + BytePos(offset);
            let hi = lo + BytePos(length);

            if !self.reported.insert(KeySourceRange { lo: lo.0, hi: hi.0 }) {
                continue;
            }

            Violation {
                owner: expression.hir_id,
                span: Span::with_root_ctxt(lo, hi),
                reason,
            }
            .emit(cx);
        }
    }
}
