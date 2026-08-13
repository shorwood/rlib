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

#[derive(Clone)]
/// Carries one manual error candidate found by this analysis.
pub struct Candidate {
    /// Stores the `span` value used by this analysis.
    pub(crate) span: Span,
    /// Stores the `name` value used by this analysis.
    pub(crate) name: String,
    /// Stores the `message` value used by this analysis.
    pub(super) message: String,
    /// Stores the `source_field` value used by this analysis.
    pub(crate) source_field: Option<String>,
}

/// Identifies one authored error type and its declaration span.
struct ErrorType {
    /// Declaration span.
    span: Span,
    /// Authored type name.
    name: String,
}

#[derive(Default)]
/// Collects manual error contracts across the crate.
pub struct Catalog {
    /// Stores the `displays` value used by this analysis.
    displays: HashMap<LocalDefId, String>,
    /// Stores the `errors` value used by this analysis.
    errors: HashMap<LocalDefId, ErrorSource>,
    /// Stores the `types` value used by this analysis.
    types: HashMap<LocalDefId, ErrorType>,
}

impl Catalog {
    /// Performs the `candidates` operation for this value.
    pub(crate) fn candidates(&self) -> Vec<Candidate> {
        // Prepare the values used by this stage.
        let mut candidates = self
            .errors
            .iter()
            .filter_map(|(definition, source_field)| {
                let message = self.displays.get(definition)?;
                let error_type = self.types.get(definition)?;
                Some(Candidate {
                    span: error_type.span,
                    name: error_type.name.clone(),
                    message: message.clone(),
                    source_field: source_field.field(),
                })
            })
            .collect::<Vec<_>>();

        // Perform the next step of the analysis.
        candidates.sort_by_key(|analyze_candidate| analyze_candidate.span.lo());
        candidates
    }

    #[cfg(feature = "derive_more")]
    /// Performs the `contains` operation for this value.
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
        let Some(ImplContract {
            definition,
            trait_name,
        }) = ImplContract::for_item(cx, item, implementation)
        else {
            return;
        };
        if trait_name == "Display" {
            if let Some(message) = display_message(cx, item) {
                self.displays.insert(definition, message);
            }
        } else if trait_name == "Error"
            && let Some(source_field) = ErrorSource::analyze_error_source(cx, item)
        {
            self.errors.insert(definition, source_field);
        }
    }

    /// Performs the `check_item` operation for this value.
    pub(crate) fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        if item.span.from_expansion() {
            return;
        }
        match item.kind {
            ItemKind::Struct(identifier, ..) => {
                self.types.insert(
                    item.owner_id.def_id,
                    ErrorType {
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

/// Classifies `ErrorSource` cases used by this analysis.
enum ErrorSource {
    /// Represents the `Empty` case.
    Empty,
    /// Stores the `item` value used by this analysis.
    Field(
        /// Field whose value is returned as the error source.
        String,
    ),
}

impl ErrorSource {
    /// Performs the `analyze_error_source` step of the lint analysis.
    fn analyze_error_source(cx: &LateContext<'_>, item: &Item<'_>) -> Option<Self> {
        // Prepare the values used by this stage.
        let source = AuthoredItemSource::for_item(cx, item)?;
        let implementation = match syn::parse_str::<syn::ItemImpl>(&source) {
            Ok(implementation) => implementation,
            Err(_error) => return None,
        };

        // Classify the current analyze_candidate.
        match implementation.items.as_slice() {
            [] => Some(Self::Empty),
            [syn::ImplItem::Fn(method)] => conventional_source(method).map(ErrorSource::Field),
            _ => None,
        }
    }
}

impl ErrorSource {
    /// Performs the `field` operation for this value.
    fn field(&self) -> Option<String> {
        match self {
            Self::Empty => None,
            Self::Field(field) => Some(field.clone()),
        }
    }
}

/// Connects a relevant standard trait implementation to its local type.
struct ImplContract {
    /// Local type receiving the implementation.
    definition: LocalDefId,
    /// Standard error trait implemented by the item.
    trait_name: &'static str,
}

impl ImplContract {
    /// Extracts a relevant standard trait contract from an implementation.
    fn for_item(
        cx: &LateContext<'_>,
        item: &Item<'_>,
        implementation: &rustc_hir::Impl<'_>,
    ) -> Option<Self> {
        // Reject inputs that do not satisfy this stage.
        if !cx.tcx.hir_attrs(item.hir_id()).is_empty() {
            return None;
        }
        let trait_id = implementation
            .of_trait
            .and_then(|trait_ref| trait_ref.trait_ref.trait_def_id())?;

        // Reject inputs that do not satisfy this stage.
        if !matches!(cx.tcx.crate_name(trait_id.krate).as_str(), "core" | "std") {
            return None;
        }

        // Prepare the values used by this stage.
        let trait_name = match cx.tcx.item_name(trait_id).as_str() {
            "Display" => "Display",
            "Error" => "Error",
            _ => return None,
        };

        // Prepare the values used by this stage.
        let trait_ref = cx
            .tcx
            .impl_trait_ref(item.owner_id.def_id)
            .instantiate_identity();
        let ty::Adt(definition, _) = trait_ref.self_ty().kind() else {
            return None;
        };

        // Reject inputs that do not satisfy this stage.
        if !definition.is_struct() || !cx.tcx.generics_of(definition.did()).own_params.is_empty() {
            return None;
        }
        Some(Self {
            definition: definition.did().as_local()?,
            trait_name,
        })
    }
}

/// Performs the `display_message` step of the lint analysis.
fn display_message(cx: &LateContext<'_>, item: &Item<'_>) -> Option<String> {
    // Prepare the values used by this stage.
    let source = AuthoredItemSource::for_item(cx, item)?;
    let implementation = match syn::parse_str::<syn::ItemImpl>(&source) {
        Ok(implementation) => implementation,
        Err(_error) => return None,
    };

    // Prepare the values used by this stage.
    let [syn::ImplItem::Fn(method)] = implementation.items.as_slice() else {
        return None;
    };
    if method.sig.ident != "fmt" {
        return None;
    }
    let mut inputs = method.sig.inputs.iter();

    // Reject inputs that do not satisfy this stage.
    if !matches!(inputs.next(), Some(syn::FnArg::Receiver(_))) {
        return None;
    }
    let Some(syn::FnArg::Typed(formatter)) = inputs.next() else {
        return None;
    };

    // Reject inputs that do not satisfy this stage.
    if inputs.next().is_some() {
        return None;
    }
    let syn::Pat::Ident(formatter) = formatter.pat.as_ref() else {
        return None;
    };

    // Prepare the values used by this stage.
    let [syn::Stmt::Expr(syn::Expr::MethodCall(call), _)] = method.block.stmts.as_slice() else {
        return None;
    };
    if call.method != "write_str" || call.turbofish.is_some() || call.args.len() != 1 {
        return None;
    }
    if !matches!(call.receiver.as_ref(), syn::Expr::Path(path) if path.path.is_ident(&formatter.ident))
    // Perform the next step of the analysis.
    {
        return None;
    }
    let syn::Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Str(message),
        ..
    }) = call.args.first()?
    // Perform the next step of the analysis.
    else {
        return None;
    };
    let message = message.value();
    (!message.contains('{') && !message.contains('}')).then_some(message)
}

/// Performs the `conventional_source` step of the lint analysis.
fn conventional_source(method: &syn::ImplItemFn) -> Option<String> {
    // Reject inputs that do not satisfy this stage.
    if method.sig.ident != "source" {
        return None;
    }
    let [syn::Stmt::Expr(syn::Expr::Call(call), _)] = method.block.stmts.as_slice() else {
        return None;
    };

    // Reject inputs that do not satisfy this stage.
    if !matches!(call.func.as_ref(), syn::Expr::Path(path) if path.path.is_ident("Some")) {
        return None;
    }
    if call.args.len() != 1 {
        return None;
    }
    let argument = call.args.first()?;

    // Prepare the values used by this stage.
    let syn::Expr::Reference(reference) = argument else {
        return None;
    };
    let syn::Expr::Field(field) = reference.expr.as_ref() else {
        return None;
    };

    // Reject inputs that do not satisfy this stage.
    if !matches!(field.base.as_ref(), syn::Expr::Path(path) if path.path.is_ident("self")) {
        return None;
    }
    match &field.member {
        syn::Member::Named(name) => Some(name.to_string()),
        syn::Member::Unnamed(_) => None,
    }
}
