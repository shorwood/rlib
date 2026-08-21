extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_span;

use std::collections::HashMap;

use rustc_hir::def::{CtorOf, DefKind, Res};
use rustc_hir::{ExprKind, ImplItem, ImplItemKind, Item, ItemKind, Mutability, PatKind};
use rustc_lint::LateContext;
use rustc_middle::ty;
use rustc_span::def_id::LocalDefId;
use rustc_span::{Span, sym};

use crate::utils::direct_forwarding::DirectForwarding;
use crate::utils::source_provenance::AuthoredItemSource;

// -----------------------------------------------------------------------------
// ManualErrorCandidate: Derivable error contract
// -----------------------------------------------------------------------------

/// Manual `Display` and `Error` contract reproducible by a derive.
#[derive(Clone)]
pub struct ManualErrorCandidate {
    /// Authored error type receiving a diagnostic.
    pub(crate) span: Span,
    /// Error type name.
    pub(crate) name: String,
    /// Static message returned by the manual `Display` implementation.
    pub message: String,
    /// Field returned by `Error::source`, when present.
    pub(crate) source_field: Option<String>,
}

/// Identifies one authored error type and its declaration span.
struct ManualErrorCandidateType {
    /// Declaration span.
    span: Span,
    /// Authored type name.
    name: String,
}

// -----------------------------------------------------------------------------
// ManualErrorCatalog: Cross-implementation correlation
// -----------------------------------------------------------------------------

/// Collects manual error contracts across the crate.
#[derive(Default)]
pub struct ManualErrorCatalog {
    /// Static display messages indexed by their local error type.
    displays: HashMap<LocalDefId, String>,
    /// Standard source behavior indexed by its local error type.
    errors: HashMap<LocalDefId, ManualErrorSource>,
    /// Authored error declarations indexed by local definition.
    types: HashMap<LocalDefId, ManualErrorCandidateType>,
}

impl ManualErrorCatalog {
    /// Returns complete manual contracts in source order.
    pub(crate) fn candidates(&self) -> Vec<ManualErrorCandidate> {
        let mut candidates = self
            .errors
            .iter()
            .filter_map(|(definition, source_field)| {
                let message = self.displays.get(definition)?;
                let error_type = self.types.get(definition)?;
                Some(ManualErrorCandidate {
                    span: error_type.span,
                    name: error_type.name.clone(),
                    message: message.clone(),
                    source_field: source_field.field(),
                })
            })
            .collect::<Vec<_>>();

        candidates.sort_by_key(|candidate| candidate.span.lo());
        candidates
    }

    /// Returns whether a type manually implements both presentation and error behavior.
    #[cfg(feature = "derive_more")]
    pub(crate) fn contains(&self, definition: LocalDefId) -> bool {
        self.displays.contains_key(&definition) && self.errors.contains_key(&definition)
    }

    /// Records a relevant `Display` or `Error` implementation.
    fn record_impl(
        &mut self,
        cx: &LateContext<'_>,
        item: &Item<'_>,
        implementation: &rustc_hir::Impl<'_>,
    ) {
        // Only standard `Display` and `Error` implementations contribute to this catalog.
        let Some(ManualErrorImpl {
            definition,
            trait_name,
        }) = ManualErrorImpl::for_item(cx, item, implementation)
        else {
            return;
        };
        if trait_name == "Display" {
            if let Some(message) = ManualErrorImpl::display_message(cx, item) {
                self.displays.insert(definition, message);
            }
        } else if trait_name == "Error"
            && let Some(source_field) = ManualErrorSource::from_item(cx, item)
        {
            self.errors.insert(definition, source_field);
        }
    }

    /// Records an authored error type or one of its relevant trait implementations.
    pub(crate) fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Macro-expanded declarations are not reliable authored error evidence.
        if item.span.from_expansion() {
            return;
        }
        match item.kind {
            ItemKind::Struct(identifier, ..) => {
                self.types.insert(
                    item.owner_id.def_id,
                    ManualErrorCandidateType {
                        span: item.span,
                        name: identifier.name.to_string(),
                    },
                );
            }
            ItemKind::Impl(implementation) => {
                self.record_impl(cx, item, &implementation);
            }
            _ => {}
        }
    }
}

// -----------------------------------------------------------------------------
// ManualErrorSource: Conventional source implementation parsing
// -----------------------------------------------------------------------------

/// Source behavior expressed by a manual standard `Error` implementation.
enum ManualErrorSource {
    /// The implementation uses the default source behavior.
    Empty,
    /// The implementation returns a reference to one named field.
    Field(
        /// Field whose value is returned as the error source.
        String,
    ),
}

impl ManualErrorSource {
    /// Parses a conventional empty or single-field source implementation.
    fn from_item(cx: &LateContext<'_>, item: &Item<'_>) -> Option<Self> {
        // Source behavior can be analyzed only from an implementation item.
        let ItemKind::Impl(implementation) = item.kind else {
            return None;
        };
        match implementation.items {
            [] => Some(Self::Empty),
            [method] => {
                let method = cx.tcx.hir_impl_item(*method);
                Self::conventional_field(cx, method)
                    .map(Self::Field)
                    .or_else(|| Self::is_conventional_none(cx, method).then_some(Self::Empty))
            }
            _ => None,
        }
    }

    /// Recognizes an explicit source method that returns the standard `Option::None`.
    fn is_conventional_none(cx: &LateContext<'_>, method: &ImplItem<'_>) -> bool {
        // Only an unannotated `source` method can use this conventional shape.
        if method.ident.name.as_str() != "source" || !cx.tcx.hir_attrs(method.hir_id()).is_empty() {
            return false;
        }

        // The source implementation must be a function body.
        let ImplItemKind::Fn(_, body_id) = method.kind else {
            return false;
        };

        // The method body must forward exactly one expression.
        let Some(expression) =
            DirectForwarding::single_body_expression(cx.tcx.hir_body(body_id).value)
        else {
            return false;
        };

        // `None` must be expressed as a direct constructor path.
        let ExprKind::Path(path) = expression.kind else {
            return false;
        };

        // Only a resolved variant constructor can be standard `Option::None`.
        let Res::Def(DefKind::Ctor(CtorOf::Variant, _), constructor) =
            cx.qpath_res(&path, expression.hir_id)
        else {
            return false;
        };
        let variant = cx.tcx.parent(constructor);
        cx.tcx.item_name(variant).as_str() == "None"
            && cx
                .tcx
                .is_diagnostic_item(sym::Option, cx.tcx.parent(variant))
    }

    /// Parses a conventional `Some(&self.field)` source method.
    fn conventional_field(cx: &LateContext<'_>, method: &ImplItem<'_>) -> Option<String> {
        // Only an unannotated `source` method can use this conventional shape.
        if method.ident.name.as_str() != "source" || !cx.tcx.hir_attrs(method.hir_id()).is_empty() {
            return None;
        }

        // The source implementation must be a function body.
        let ImplItemKind::Fn(_, body_id) = method.kind else {
            return None;
        };
        let body = cx.tcx.hir_body(body_id);

        // Conventional source methods have exactly one receiver parameter.
        let [parameter] = body.params else {
            return None;
        };

        // The receiver must remain a direct binding for field forwarding.
        let PatKind::Binding(_, receiver, _, None) = parameter.pat.kind else {
            return None;
        };
        let expression = DirectForwarding::single_body_expression(body.value)?;

        // `Some` forwarding must be a one-argument constructor call.
        let ExprKind::Call(callee, [argument]) = expression.kind else {
            return None;
        };

        // The `Some` constructor must be named by a direct path.
        let ExprKind::Path(path) = callee.kind else {
            return None;
        };

        // Only a resolved variant constructor can be standard `Option::Some`.
        let Res::Def(DefKind::Ctor(CtorOf::Variant, _), constructor) =
            cx.qpath_res(&path, callee.hir_id)
        else {
            return None;
        };
        let variant = cx.tcx.parent(constructor);

        // The wrapper must be exactly the standard `Option::Some` variant.
        if cx.tcx.item_name(variant).as_str() != "Some"
            || !cx
                .tcx
                .is_diagnostic_item(sym::Option, cx.tcx.parent(variant))
        {
            return None;
        }

        // The field must be borrowed immutably for an error-source reference.
        let ExprKind::AddrOf(_, Mutability::Not, field) = argument.kind else {
            return None;
        };

        // The borrowed value must be a direct receiver field access.
        let ExprKind::Field(base, name) = field.kind else {
            return None;
        };
        DirectForwarding::is_binding(cx, base, receiver).then(|| name.name.to_string())
    }

    /// Returns the selected field, if the implementation overrides `source`.
    fn field(&self) -> Option<String> {
        match self {
            Self::Empty => None,
            Self::Field(field) => Some(field.clone()),
        }
    }
}

// -----------------------------------------------------------------------------
// ManualErrorImpl: Standard trait implementation parsing
// -----------------------------------------------------------------------------

/// Connects a relevant standard trait implementation to its local type.
struct ManualErrorImpl {
    /// Local type receiving the implementation.
    definition: LocalDefId,
    /// Standard error trait implemented by the item.
    trait_name: &'static str,
}

impl ManualErrorImpl {
    /// Extracts a relevant standard trait contract from an implementation.
    fn for_item(
        cx: &LateContext<'_>,
        item: &Item<'_>,
        implementation: &rustc_hir::Impl<'_>,
    ) -> Option<Self> {
        // Attributes may alter implementation behavior beyond the recognized convention.
        if !cx.tcx.hir_attrs(item.hir_id()).is_empty() {
            return None;
        }
        let trait_id = implementation
            .of_trait
            .and_then(|trait_ref| trait_ref.trait_ref.trait_def_id())?;

        // Only standard-library trait implementations can be manual error contracts.
        if !matches!(cx.tcx.crate_name(trait_id.krate).as_str(), "core" | "std") {
            return None;
        }

        let trait_name = match cx.tcx.item_name(trait_id).as_str() {
            "Display" => "Display",
            "Error" => "Error",

            // Other standard traits do not supply error presentation behavior.
            _ => return None,
        };

        let trait_ref = cx
            .tcx
            .impl_trait_ref(item.owner_id.def_id)
            .instantiate_identity();

        // The implementation target must be a nominal type.
        let ty::Adt(definition, _) = trait_ref.self_ty().kind() else {
            return None;
        };

        // This recognizer supports struct errors only.
        if !definition.is_struct() {
            return None;
        }
        Some(Self {
            definition: definition.did().as_local()?,
            trait_name,
        })
    }

    /// Parses a single static `write_str` display implementation.
    fn display_message(cx: &LateContext<'_>, item: &Item<'_>) -> Option<String> {
        let source = AuthoredItemSource::for_item(cx, item)?;

        // Source text must parse as an implementation before its display shape is inspected.
        let implementation = match syn::parse_str::<syn::ItemImpl>(&source) {
            Ok(implementation) => implementation,

            // Invalid source cannot establish the conventional display shape.
            Err(_error) => return None,
        };

        // A conventional display implementation contains one method.
        let [syn::ImplItem::Fn(method)] = implementation.items.as_slice() else {
            return None;
        };

        // That method must be `Display::fmt`.
        if method.sig.ident != "fmt" {
            return None;
        }
        let mut inputs = method.sig.inputs.iter();

        // `fmt` begins with its receiver argument.
        if !matches!(inputs.next(), Some(syn::FnArg::Receiver(_))) {
            return None;
        }

        // `fmt` requires a typed formatter argument after the receiver.
        let Some(syn::FnArg::Typed(formatter)) = inputs.next() else {
            return None;
        };

        // Additional arguments are not part of the standard `fmt` signature.
        if inputs.next().is_some() {
            return None;
        }

        // The formatter argument must have an identifier pattern for receiver matching.
        let syn::Pat::Ident(formatter) = formatter.pat.as_ref() else {
            return None;
        };

        // The body must contain exactly one expression statement.
        let [syn::Stmt::Expr(syn::Expr::MethodCall(call), _)] = method.block.stmts.as_slice()
        else {
            return None;
        };

        // The body must be one non-generic `write_str` call with one argument.
        if call.method != "write_str" || call.turbofish.is_some() || call.args.len() != 1 {
            return None;
        }

        // The call must target the formatter parameter itself.
        if !matches!(call.receiver.as_ref(), syn::Expr::Path(path) if path.path.is_ident(&formatter.ident))
        {
            return None;
        }

        // The sole argument must be a static string literal.
        let syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(message),
            ..
        }) = call.args.first()?
        else {
            return None;
        };
        let message = message.value();
        (!message.contains('{') && !message.contains('}')).then_some(message)
    }
}
