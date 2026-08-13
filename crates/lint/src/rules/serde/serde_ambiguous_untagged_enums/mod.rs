extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

use rustc_errors::DiagDecorator;
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use super::contracts::{SerdeContractCatalog, serde_attributes};
use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::authored_item_source;

struct Candidate {
    definition: LocalDefId,
    span: Span,
    first: String,
    second: String,
    witness: Vec<String>,
}

struct Violation {
    span: Span,
    first: String,
    second: String,
    witness: Vec<String>,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "Serde untagged variants `{}` and `{}` accept the same input shape",
            self.first, self.second
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        let witness = if self.witness.is_empty() {
            "an empty object".to_owned()
        } else {
            format!("an object containing {}", self.witness.join(", "))
        };
        Cow::Owned(format!(
            "{witness} is a concrete witness accepted by both variants, so the first declaration wins"
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("add an explicit Serde tag or make the variants' required shapes disjoint")
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            SERDE_AMBIGUOUS_UNTAGGED_ENUMS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.span,
                    format!("`{}` shadows `{}` for this shape", self.first, self.second),
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

#[derive(Default)]
struct SerdeAmbiguousUntaggedEnums {
    catalog: SerdeContractCatalog,
    candidates: Vec<Candidate>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub SERDE_AMBIGUOUS_UNTAGGED_ENUMS,
    Warn,
    "finds overlapping Serde untagged enum variants",
    SerdeAmbiguousUntaggedEnums::default()
}

impl LateLintPass<'_> for SerdeAmbiguousUntaggedEnums {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
        if item.span.from_expansion() || !matches!(item.kind, ItemKind::Enum(..)) {
            return;
        }
        let Some(source) = authored_item_source(cx, item) else {
            return;
        };
        let Ok(enumeration) = syn::parse_str::<syn::ItemEnum>(&source) else {
            return;
        };
        let container = serde_attributes(&enumeration.attrs);
        if !container.untagged {
            return;
        }
        let variants = enumeration
            .variants
            .iter()
            .filter_map(|variant| variant_shape(variant, container.deny_unknown_fields))
            .collect::<Vec<_>>();
        for (index, first) in variants.iter().enumerate() {
            for second in &variants[index + 1..] {
                let Some(witness) = overlap_witness(first, second) else {
                    continue;
                };
                self.candidates.push(Candidate {
                    definition: item.owner_id.def_id,
                    span: item.span,
                    first: first.name.clone(),
                    second: second.name.clone(),
                    witness,
                });
            }
        }
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for candidate in self.candidates.drain(..) {
            if self
                .catalog
                .derived_type(candidate.definition, "Deserialize")
                .is_none()
            {
                continue;
            }
            Violation {
                span: candidate.span,
                first: candidate.first,
                second: candidate.second,
                witness: candidate
                    .witness
                    .into_iter()
                    .map(|key| format!("`{key}`"))
                    .collect(),
            }
            .emit(cx);
        }
    }
}

struct VariantShape {
    name: String,
    fields: BTreeMap<String, &'static str>,
    required: BTreeSet<String>,
    closed: bool,
}

fn variant_shape(variant: &syn::Variant, closed: bool) -> Option<VariantShape> {
    let syn::Fields::Named(fields) = &variant.fields else {
        return None;
    };
    let mut shape = VariantShape {
        name: variant.ident.to_string(),
        fields: BTreeMap::new(),
        required: BTreeSet::new(),
        closed,
    };
    for field in &fields.named {
        let attributes = serde_attributes(&field.attrs);
        if attributes.skip_deserialize {
            continue;
        }
        let rust_name = field.ident.as_ref()?.to_string();
        let name = attributes.rename_deserialize.unwrap_or(rust_name);
        shape.fields.insert(name.clone(), scalar_domain(&field.ty)?);
        if !attributes.has_default && !is_option(&field.ty) {
            shape.required.insert(name);
        }
    }
    Some(shape)
}

fn overlap_witness(first: &VariantShape, second: &VariantShape) -> Option<Vec<String>> {
    let required = first
        .required
        .union(&second.required)
        .cloned()
        .collect::<BTreeSet<_>>();
    if first.closed && !required.is_subset(&first.fields.keys().cloned().collect())
        || second.closed && !required.is_subset(&second.fields.keys().cloned().collect())
    {
        return None;
    }
    for key in &required {
        if let (Some(first_domain), Some(second_domain)) =
            (first.fields.get(key), second.fields.get(key))
            && first_domain != second_domain
        {
            return None;
        }
    }
    Some(required.into_iter().collect())
}

fn scalar_domain(ty: &syn::Type) -> Option<&'static str> {
    let syn::Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    match segment.ident.to_string().as_str() {
        "bool" => Some("bool"),
        "char" => Some("char"),
        "String" | "str" => Some("string"),
        "i8" => Some("i8"),
        "i16" => Some("i16"),
        "i32" => Some("i32"),
        "i64" => Some("i64"),
        "i128" => Some("i128"),
        "isize" => Some("isize"),
        "u8" => Some("u8"),
        "u16" => Some("u16"),
        "u32" => Some("u32"),
        "u64" => Some("u64"),
        "u128" => Some("u128"),
        "usize" => Some("usize"),
        "f32" => Some("f32"),
        "f64" => Some("f64"),
        "Option" => match &segment.arguments {
            syn::PathArguments::AngleBracketed(arguments) => {
                arguments.args.iter().find_map(|arg| {
                    if let syn::GenericArgument::Type(inner) = arg {
                        scalar_domain(inner)
                    } else {
                        None
                    }
                })
            }
            _ => None,
        },
        _ => None,
    }
}

fn is_option(ty: &syn::Type) -> bool {
    matches!(ty, syn::Type::Path(path)
        if path.path.segments.last().is_some_and(|segment| segment.ident == "Option"))
}
