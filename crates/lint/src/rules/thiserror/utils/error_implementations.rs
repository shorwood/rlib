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
        let ItemKind::Impl(implementation) = item.kind else {
            return None;
        };
        match implementation.items {
            [] => Some(Self::Empty),
            [method] => {
                let method = cx.tcx.hir_impl_item(*method);
                Self::conventional_field(cx, method)
                    .map(Self::Field)
                    .or_else(|| Self::conventional_none(cx, method).then_some(Self::Empty))
            }
            _ => None,
        }
    }

    /// Recognizes an explicit source method that returns the standard `Option::None`.
    fn conventional_none(cx: &LateContext<'_>, method: &ImplItem<'_>) -> bool {
        if method.ident.name.as_str() != "source" || !cx.tcx.hir_attrs(method.hir_id()).is_empty() {
            return false;
        }
        let ImplItemKind::Fn(_, body_id) = method.kind else {
            return false;
        };
        let Some(expression) =
            DirectForwarding::single_body_expression(cx.tcx.hir_body(body_id).value)
        else {
            return false;
        };
        let ExprKind::Path(path) = expression.kind else {
            return false;
        };
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
        if method.ident.name.as_str() != "source" || !cx.tcx.hir_attrs(method.hir_id()).is_empty() {
            return None;
        }
        let ImplItemKind::Fn(_, body_id) = method.kind else {
            return None;
        };
        let body = cx.tcx.hir_body(body_id);
        let [parameter] = body.params else {
            return None;
        };
        let PatKind::Binding(_, receiver, _, None) = parameter.pat.kind else {
            return None;
        };
        let expression = DirectForwarding::single_body_expression(body.value)?;
        let ExprKind::Call(callee, [argument]) = expression.kind else {
            return None;
        };
        let ExprKind::Path(path) = callee.kind else {
            return None;
        };
        let Res::Def(DefKind::Ctor(CtorOf::Variant, _), constructor) =
            cx.qpath_res(&path, callee.hir_id)
        else {
            return None;
        };
        let variant = cx.tcx.parent(constructor);
        if cx.tcx.item_name(variant).as_str() != "Some"
            || !cx
                .tcx
                .is_diagnostic_item(sym::Option, cx.tcx.parent(variant))
        {
            return None;
        }
        let ExprKind::AddrOf(_, Mutability::Not, field) = argument.kind else {
            return None;
        };
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
        if !cx.tcx.hir_attrs(item.hir_id()).is_empty() {
            return None;
        }
        let trait_id = implementation
            .of_trait
            .and_then(|trait_ref| trait_ref.trait_ref.trait_def_id())?;

        if !matches!(cx.tcx.crate_name(trait_id.krate).as_str(), "core" | "std") {
            return None;
        }

        let trait_name = match cx.tcx.item_name(trait_id).as_str() {
            "Display" => "Display",
            "Error" => "Error",
            _ => return None,
        };

        let trait_ref = cx
            .tcx
            .impl_trait_ref(item.owner_id.def_id)
            .instantiate_identity();
        let ty::Adt(definition, _) = trait_ref.self_ty().kind() else {
            return None;
        };

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
        let implementation = match syn::parse_str::<syn::ItemImpl>(&source) {
            Ok(implementation) => implementation,
            Err(_error) => return None,
        };

        let [syn::ImplItem::Fn(method)] = implementation.items.as_slice() else {
            return None;
        };
        if method.sig.ident != "fmt" {
            return None;
        }
        let mut inputs = method.sig.inputs.iter();

        if !matches!(inputs.next(), Some(syn::FnArg::Receiver(_))) {
            return None;
        }
        let Some(syn::FnArg::Typed(formatter)) = inputs.next() else {
            return None;
        };

        if inputs.next().is_some() {
            return None;
        }
        let syn::Pat::Ident(formatter) = formatter.pat.as_ref() else {
            return None;
        };

        let [syn::Stmt::Expr(syn::Expr::MethodCall(call), _)] = method.block.stmts.as_slice()
        else {
            return None;
        };
        if call.method != "write_str" || call.turbofish.is_some() || call.args.len() != 1 {
            return None;
        }
        if !matches!(call.receiver.as_ref(), syn::Expr::Path(path) if path.path.is_ident(&formatter.ident))
        {
            return None;
        }
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
