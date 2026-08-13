extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_span;

use std::collections::HashMap;

use rustc_hir::{Item, ItemKind};
use rustc_lint::LateContext;
use rustc_middle::ty;
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

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
    pub(super) message: String,
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

    #[cfg(feature = "derive_more")]
    /// Returns whether a type manually implements both presentation and error behavior.
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
            if let Some(message) = manual_error_impl_display_message(cx, item) {
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
        let source = AuthoredItemSource::for_item(cx, item)?;
        let implementation = match syn::parse_str::<syn::ItemImpl>(&source) {
            Ok(implementation) => implementation,
            Err(_error) => return None,
        };

        match implementation.items.as_slice() {
            [] => Some(Self::Empty),
            [syn::ImplItem::Fn(method)] => {
                manual_error_impl_conventional_source(method).map(Self::Field)
            }
            _ => None,
        }
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

        if !definition.is_struct() || !cx.tcx.generics_of(definition.did()).own_params.is_empty() {
            return None;
        }
        Some(Self {
            definition: definition.did().as_local()?,
            trait_name,
        })
    }
}

/// Parses a single static `write_str` display implementation.
fn manual_error_impl_display_message(cx: &LateContext<'_>, item: &Item<'_>) -> Option<String> {
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

    let [syn::Stmt::Expr(syn::Expr::MethodCall(call), _)] = method.block.stmts.as_slice() else {
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

/// Parses a conventional `Some(&self.field)` source method.
fn manual_error_impl_conventional_source(method: &syn::ImplItemFn) -> Option<String> {
    if method.sig.ident != "source" {
        return None;
    }
    let [syn::Stmt::Expr(syn::Expr::Call(call), _)] = method.block.stmts.as_slice() else {
        return None;
    };

    if !matches!(call.func.as_ref(), syn::Expr::Path(path) if path.path.is_ident("Some")) {
        return None;
    }
    if call.args.len() != 1 {
        return None;
    }
    let argument = call.args.first()?;

    let syn::Expr::Reference(reference) = argument else {
        return None;
    };
    let syn::Expr::Field(field) = reference.expr.as_ref() else {
        return None;
    };

    if !matches!(field.base.as_ref(), syn::Expr::Path(path) if path.path.is_ident("self")) {
        return None;
    }
    match &field.member {
        syn::Member::Named(name) => Some(name.to_string()),
        syn::Member::Unnamed(_) => None,
    }
}
