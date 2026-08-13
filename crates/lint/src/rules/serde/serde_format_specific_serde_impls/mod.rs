extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty;
use rustc_span::Span;

use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::authored_item_source;

struct Violation {
    owner: rustc_hir::HirId,
    span: Span,
    trait_name: String,
    type_name: String,
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

struct SerdeFormatSpecificSerdeImpls;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub SERDE_FORMAT_SPECIFIC_SERDE_IMPLS,
    Warn,
    "finds format-specific behavior hidden in generic Serde implementations",
    SerdeFormatSpecificSerdeImpls
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
        let Some(source) = authored_item_source(cx, item) else {
            return;
        };
        if source.lines().any(|line| {
            let line = line.trim_start();
            line.starts_with("///") || line.starts_with("#[doc")
        }) {
            return;
        }
        let Some(evidence) = format_specific_evidence(&source) else {
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

fn format_specific_evidence(source: &str) -> Option<String> {
    const FORMAT_CRATES: &[&str] = &[
        "serde_json::",
        "serde_yaml::",
        "serde_cbor::",
        "toml::",
        "bincode::",
        "rmp_serde::",
    ];
    if let Some(format) = FORMAT_CRATES
        .iter()
        .find(|format| source.contains(**format))
    {
        return Some(format!(
            "the generic implementation directly depends on the `{}` format API",
            format.trim_end_matches("::")
        ));
    }
    if !source.contains("is_human_readable()") {
        return None;
    }
    let string_shape = ["serialize_str", "deserialize_str", "deserialize_string"]
        .iter()
        .any(|operation| source.contains(operation));
    let binary_shape = [
        "serialize_u",
        "serialize_i",
        "serialize_bytes",
        "serialize_seq",
        "serialize_map",
        "deserialize_u",
        "deserialize_i",
        "deserialize_bytes",
        "deserialize_seq",
        "deserialize_map",
    ]
    .iter()
    .any(|operation| source.contains(operation));
    (string_shape && binary_shape).then(|| {
        "`is_human_readable()` selects different string and binary Serde data-model shapes"
            .to_owned()
    })
}
