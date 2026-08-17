extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{BodyId, Expr, ExprKind, Item, ItemKind, Path};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::{self, TyCtxt};
use rustc_span::Span;

use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::AuthoredItemSource;

// -----------------------------------------------------------------------------
// Violation: Format-specific Serde implementation
// -----------------------------------------------------------------------------

/// Generic Serde implementation coupled to one concrete data format.
struct Violation {
    /// Declaration whose lint level governs this finding.
    owner: rustc_hir::HirId,
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Serde trait whose implementation depends on a particular data format.
    trait_name: String,
    /// Type name quoted in the diagnostic.
    type_name: String,
    /// Format-specific operation found inside the generic Serde implementation.
    evidence: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "generic Serde `{}` for `{}` embeds format-specific behavior",
            self.trait_name, self.type_name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(self.evidence.clone())
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "use one portable Serde shape, or move this policy into a documented format-specific wrapper",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            SERDE_FORMAT_SPECIFIC_SERDE_IMPLS,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.span,
                    "this generic trait implementation has a hidden format boundary",
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// FormatEvidence: Concrete wire-format coupling
// -----------------------------------------------------------------------------

/// Format-specific operations collected from one Serde implementation.
struct FormatEvidence<'tcx> {
    /// Compiler context used to resolve referenced crates and methods.
    tcx: TyCtxt<'tcx>,
    /// Current nested body owner used for type-dependent method resolution.
    body_owner: Option<LocalDefId>,
    /// Concrete serialization format referenced by the implementation.
    format: Option<String>,
    /// Whether the implementation branches on Serde's readability mode.
    is_human_readable: bool,
    /// Whether a string-shaped serialization operation appears.
    has_string_shape: bool,
    /// Whether a binary-shaped serialization operation appears.
    has_binary_shape: bool,
}

impl<'tcx> FormatEvidence<'tcx> {
    /// Starts collecting format evidence for one implementation.
    const fn new(tcx: TyCtxt<'tcx>) -> Self {
        Self {
            tcx,
            body_owner: None,
            format: None,
            is_human_readable: false,
            has_string_shape: false,
            has_binary_shape: false,
        }
    }

    /// Converts accumulated evidence into a diagnostic explanation.
    fn finish(self) -> Option<String> {
        if let Some(format) = self.format {
            return Some(format!(
                "the generic implementation directly depends on the `{format}` format API"
            ));
        }
        (self.is_human_readable && self.has_string_shape && self.has_binary_shape).then(|| {
            "`is_human_readable()` selects different string and binary Serde data-model shapes"
                .to_owned()
        })
    }
}

impl<'tcx> Visitor<'tcx> for FormatEvidence<'tcx> {
    fn visit_nested_body(&mut self, body: BodyId) {
        let previous = self
            .body_owner
            .replace(self.tcx.hir_body_owner_def_id(body));
        self.visit_body(self.tcx.hir_body(body));
        self.body_owner = previous;
    }

    fn visit_path(&mut self, path: &Path<'tcx>, _: rustc_hir::HirId) {
        /// Serialization crates that make a generic Serde implementation format-specific.
        const FORMATS: &[&str] = &[
            "serde_json",
            "serde_yaml",
            "serde_cbor",
            "toml",
            "bincode",
            "rmp_serde",
        ];
        if let Some(definition) = path.res.opt_def_id() {
            let krate = self.tcx.crate_name(definition.krate).to_string();
            if FORMATS.contains(&krate.as_str()) {
                self.format.get_or_insert(krate);
            }
        }
        intravisit::walk_path(self, path);
    }

    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        if let ExprKind::MethodCall(segment, ..) = expression.kind
            && let Some(owner) = self.body_owner
            && let Some(definition) = self
                .tcx
                .typeck(owner)
                .type_dependent_def_id(expression.hir_id)
            && matches!(
                self.tcx.crate_name(definition.krate).as_str(),
                "serde" | "serde_core"
            )
        {
            let operation = segment.ident.name.as_str();
            self.is_human_readable |= operation == "is_human_readable";
            self.has_string_shape |= matches!(
                operation,
                "serialize_str" | "deserialize_str" | "deserialize_string"
            );
            self.has_binary_shape |= operation.starts_with("serialize_u")
                || operation.starts_with("serialize_i")
                || operation.starts_with("deserialize_u")
                || operation.starts_with("deserialize_i")
                || matches!(
                    operation,
                    "serialize_bytes"
                        | "serialize_seq"
                        | "serialize_map"
                        | "deserialize_bytes"
                        | "deserialize_seq"
                        | "deserialize_map"
                );
        }
        intravisit::walk_expr(self, expression);
    }
}

// -----------------------------------------------------------------------------
// SerdeFormatSpecificSerdeImpls: Format-neutral data-model policy
// -----------------------------------------------------------------------------

/// Rejects generic Serde implementations coupled to a concrete wire format.
struct SerdeFormatSpecificSerdeImpls;

impl SerdeFormatSpecificSerdeImpls {
    /// Returns whether documentation establishes an intentional format-specific contract.
    fn documents_format_policy(source: &str) -> bool {
        source
            .lines()
            .filter_map(|line| {
                let line = line.trim_start();
                line.strip_prefix("///")
                    .or_else(|| line.strip_prefix("#[doc"))
            })
            .any(|line| {
                let line = line.to_ascii_lowercase();
                [
                    "format",
                    "human-readable",
                    "binary",
                    "json",
                    "yaml",
                    "cbor",
                    "bincode",
                ]
                .iter()
                .any(|term| line.contains(term))
                    && ["representation", "schema", "wire", "compatib", "migration"]
                        .iter()
                        .any(|term| line.contains(term))
            })
    }
}

impl LateLintPass<'_> for SerdeFormatSpecificSerdeImpls {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        let ItemKind::Impl(implementation) = item.kind else {
            return;
        };
        if item.span.from_expansion() {
            return;
        }

        let Some(trait_id) = implementation
            .of_trait
            .and_then(|trait_ref| trait_ref.trait_ref.trait_def_id())
        else {
            return;
        };
        let trait_name = cx.tcx.item_name(trait_id).to_string();

        if !matches!(trait_name.as_str(), "Serialize" | "Deserialize")
            || !matches!(
                cx.tcx.crate_name(trait_id.krate).as_str(),
                "serde" | "serde_core"
            )
        {
            return;
        }
        let Some(source) = AuthoredItemSource::for_item(cx, item) else {
            return;
        };

        if Self::documents_format_policy(&source) {
            return;
        }

        let mut visitor = FormatEvidence::new(cx.tcx);
        for reference in implementation.items {
            visitor.visit_impl_item(cx.tcx.hir_impl_item(*reference));
        }
        let Some(evidence) = visitor.finish() else {
            return;
        };
        let trait_ref = cx
            .tcx
            .impl_trait_ref(item.owner_id.def_id)
            .instantiate_identity();

        let type_name = match trait_ref.self_ty().kind() {
            ty::Adt(definition, _) => cx.tcx.item_name(definition.did()).to_string(),
            _ => trait_ref.self_ty().to_string(),
        };

        Violation {
            owner: item.hir_id(),
            span: item.span,
            trait_name,
            type_name,
            evidence,
        }
        .emit(cx);
    }
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub SERDE_FORMAT_SPECIFIC_SERDE_IMPLS,
    Warn,
    "finds format-specific behavior hidden in generic Serde implementations",
    SerdeFormatSpecificSerdeImpls
}
