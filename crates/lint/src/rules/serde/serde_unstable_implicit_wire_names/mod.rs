extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use super::contracts::{SerdeContractCatalog, SerdeFlag, serde_attributes};
use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::authored_item_source;

struct Candidate {
    definition: LocalDefId,
    span: Span,
    serialize_names: Vec<String>,
    deserialize_names: Vec<String>,
}

struct Violation {
    span: Span,
    directions: String,
    names: Vec<String>,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "public Serde {} contract relies on implicit wire names",
            self.directions
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "renaming the Rust identifiers {} would silently change externally observed data",
            self.names.join(", ")
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "declare `#[serde(rename_all = \"...\")]` or explicit member renames for each wire direction",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            SERDE_UNSTABLE_IMPLICIT_WIRE_NAMES,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this public contract inherits Rust identifiers");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

#[derive(Default)]
struct SerdeUnstableImplicitWireNames {
    catalog: SerdeContractCatalog,
    candidates: Vec<Candidate>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub SERDE_UNSTABLE_IMPLICIT_WIRE_NAMES,
    Warn,
    "finds public Serde contracts with implicit wire names",
    SerdeUnstableImplicitWireNames::default()
}

impl LateLintPass<'_> for SerdeUnstableImplicitWireNames {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
        if item.span.from_expansion()
            || !matches!(item.kind, ItemKind::Struct(..) | ItemKind::Enum(..))
        {
            return;
        }
        let Some(source) = authored_item_source(cx, item) else {
            return;
        };
        let Some((is_public, serialize_names, deserialize_names)) = contract_names(&source) else {
            return;
        };
        if !is_public || serialize_names.is_empty() && deserialize_names.is_empty() {
            return;
        }
        self.candidates.push(Candidate {
            definition: item.owner_id.def_id,
            span: item.span,
            serialize_names,
            deserialize_names,
        });
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for candidate in self.candidates.drain(..) {
            let serializes = self
                .catalog
                .derived_type(candidate.definition, "Serialize")
                .is_some();
            let deserializes = self
                .catalog
                .derived_type(candidate.definition, "Deserialize")
                .is_some();
            let mut names = Vec::new();
            let mut directions = Vec::new();
            if serializes && !candidate.serialize_names.is_empty() {
                directions.push("serialization");
                names.extend(candidate.serialize_names);
            }
            if deserializes && !candidate.deserialize_names.is_empty() {
                directions.push("deserialization");
                names.extend(candidate.deserialize_names);
            }
            names.sort();
            names.dedup();
            if directions.is_empty() {
                continue;
            }
            Violation {
                span: candidate.span,
                directions: directions.join(" and "),
                names: names.into_iter().map(|name| format!("`{name}`")).collect(),
            }
            .emit(cx);
        }
    }
}

fn contract_names(source: &str) -> Option<(bool, Vec<String>, Vec<String>)> {
    if let Ok(structure) = syn::parse_str::<syn::ItemStruct>(source) {
        let container = serde_attributes(&structure.attrs);
        let fields = structure.fields.iter().filter_map(|field| {
            field
                .ident
                .as_ref()
                .map(|name| (name.to_string(), serde_attributes(&field.attrs)))
        });
        let (serialize, deserialize) = implicit_names(
            fields,
            container.rename_all_serialize.as_deref(),
            container.rename_all_deserialize.as_deref(),
        );
        return Some((
            matches!(structure.vis, syn::Visibility::Public(_)),
            serialize,
            deserialize,
        ));
    }
    let enumeration = syn::parse_str::<syn::ItemEnum>(source).ok()?;
    let container = serde_attributes(&enumeration.attrs);
    if container.has(SerdeFlag::Untagged) {
        return None;
    }
    let variants = enumeration
        .variants
        .iter()
        .map(|variant| (variant.ident.to_string(), serde_attributes(&variant.attrs)));
    let (serialize, deserialize) = implicit_names(
        variants,
        container.rename_all_serialize.as_deref(),
        container.rename_all_deserialize.as_deref(),
    );
    Some((
        matches!(enumeration.vis, syn::Visibility::Public(_)),
        serialize,
        deserialize,
    ))
}

fn implicit_names(
    members: impl Iterator<Item = (String, super::contracts::SerdeAttributes)>,
    serialize_rule: Option<&str>,
    deserialize_rule: Option<&str>,
) -> (Vec<String>, Vec<String>) {
    let mut serialize = Vec::new();
    let mut deserialize = Vec::new();
    for (name, attributes) in members {
        if serialize_rule.is_none()
            && attributes.rename_serialize.is_none()
            && !attributes.has(SerdeFlag::SkipSerialize)
        {
            serialize.push(name.clone());
        }
        if deserialize_rule.is_none()
            && attributes.rename_deserialize.is_none()
            && !attributes.has(SerdeFlag::SkipDeserialize)
        {
            deserialize.push(name);
        }
    }
    (serialize, deserialize)
}
