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

use super::utils::contracts::{SerdeAttributes, SerdeContractCatalog, SerdeFlag};
use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::AuthoredItemSource;

// -----------------------------------------------------------------------------
// Violation: Ambiguous untagged enum representation
// -----------------------------------------------------------------------------

/// Untagged enum awaiting complete variant-shape comparison.
struct Candidate {
    /// Local declaration identity used to associate evidence collected in separate passes.
    definition: LocalDefId,
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// First conflicting contract member.
    first: String,
    /// Second conflicting contract member.
    second: String,
    /// Concrete input shape proving the overlap.
    witness: Vec<String>,
}

/// Pair of untagged variants with a concrete overlapping input shape.
struct Violation {
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// First conflicting contract member.
    first: String,
    /// Second conflicting contract member.
    second: String,
    /// Concrete input shape proving the overlap.
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

/// Required input fields and openness of one untagged struct variant.
struct VariantShape {
    /// Authored type or member name involved in the wire contract.
    name: String,
    /// Authored fields relevant to the contract.
    fields: BTreeMap<String, &'static str>,
    /// Wire fields that must be present for this variant to deserialize.
    required: BTreeSet<String>,
    /// Whether unknown fields are rejected instead of ignored.
    is_closed: bool,
}

impl VariantShape {
    /// Builds the accepted object shape for one named-field variant.
    fn from_variant(variant: &syn::Variant) -> Option<Self> {
        let syn::Fields::Named(fields) = &variant.fields else {
            return None;
        };

        let mut shape = Self {
            name: variant.ident.to_string(),
            fields: BTreeMap::new(),
            required: BTreeSet::new(),
            is_closed: false,
        };

        for field in &fields.named {
            let attributes = SerdeAttributes::from_attributes(&field.attrs);
            if attributes.has(SerdeFlag::SkipDeserialize) {
                continue;
            }
            let rust_name = field.ident.as_ref()?.to_string();
            let has_default = attributes.has(SerdeFlag::HasDefault);
            let name = attributes.rename_deserialize.unwrap_or(rust_name);

            shape.fields.insert(
                name.clone(),
                SerdeAmbiguousUntaggedEnums::scalar_domain(&field.ty)?,
            );
            if !(!has_default && !SerdeAmbiguousUntaggedEnums::is_option(&field.ty)) {
                continue;
            }
            shape.required.insert(name);
        }

        Some(shape)
    }

    /// Returns a concrete field set accepted by both variant shapes.
    fn overlap_witness(&self, second: &Self) -> Option<Vec<String>> {
        let required = self
            .required
            .union(&second.required)
            .cloned()
            .collect::<BTreeSet<_>>();
        if (self.is_closed && !required.is_subset(&self.fields.keys().cloned().collect()))
            || (second.is_closed && !required.is_subset(&second.fields.keys().cloned().collect()))
        {
            return None;
        }

        for key in &required {
            if let (Some(first_domain), Some(second_domain)) =
                (self.fields.get(key), second.fields.get(key))
                && first_domain != second_domain
            {
                return None;
            }
        }

        Some(required.into_iter().collect())
    }
}

// -----------------------------------------------------------------------------
// SerdeAmbiguousUntaggedEnums: Unambiguous wire-shape policy
// -----------------------------------------------------------------------------

/// Finds concrete input shapes accepted by more than one untagged variant.
#[derive(Default)]
struct SerdeAmbiguousUntaggedEnums {
    /// Effective Serde contracts consulted after all local declarations are known.
    catalog: SerdeContractCatalog,
    /// Authored Serde declarations awaiting crate-wide contract comparison.
    candidates: Vec<Candidate>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub SERDE_AMBIGUOUS_UNTAGGED_ENUMS,
    Warn,
    "finds overlapping Serde untagged enum variants",
    SerdeAmbiguousUntaggedEnums::default()
}

impl SerdeAmbiguousUntaggedEnums {
    /// Recognizes signed integer scalar domains.
    fn signed_domain(name: &str) -> Option<&'static str> {
        match name {
            "i8" => Some("i8"),
            "i16" => Some("i16"),
            "i32" => Some("i32"),
            "i64" => Some("i64"),
            "i128" => Some("i128"),
            "isize" => Some("isize"),
            _ => None,
        }
    }

    /// Recognizes unsigned integer scalar domains.
    fn unsigned_domain(name: &str) -> Option<&'static str> {
        match name {
            "u8" => Some("u8"),
            "u16" => Some("u16"),
            "u32" => Some("u32"),
            "u64" => Some("u64"),
            "u128" => Some("u128"),
            "usize" => Some("usize"),
            _ => None,
        }
    }

    /// Recognizes floating-point scalar domains.
    fn float_domain(name: &str) -> Option<&'static str> {
        match name {
            "f32" => Some("f32"),
            "f64" => Some("f64"),
            _ => None,
        }
    }

    /// Classifies primitive scalar names into their overlap domains.
    fn primitive_domain(name: &str) -> Option<&'static str> {
        match name {
            "bool" => Some("bool"),
            "char" => Some("char"),
            "String" | "str" => Some("string"),
            _ => Self::signed_domain(name)
                .or_else(|| Self::unsigned_domain(name))
                .or_else(|| Self::float_domain(name)),
        }
    }

    /// Classifies scalar syntax types that can overlap during untagged deserialization.
    fn scalar_domain(ty: &syn::Type) -> Option<&'static str> {
        let syn::Type::Path(path) = ty else {
            return None;
        };
        let segment = path.path.segments.last()?;

        let name = segment.ident.to_string();
        if name != "Option" {
            return Self::primitive_domain(&name);
        }

        // Unwrap an optional scalar while preserving its overlap domain.
        let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
            return None;
        };
        arguments.args.iter().find_map(|arg| {
            if let syn::GenericArgument::Type(inner) = arg {
                Self::scalar_domain(inner)
            } else {
                None
            }
        })
    }

    /// Returns whether the type is the standard optional container.
    fn is_option(ty: &syn::Type) -> bool {
        matches!(ty, syn::Type::Path(path)
        if path.path.segments.last().is_some_and(|segment| segment.ident == "Option"))
    }
}
impl LateLintPass<'_> for SerdeAmbiguousUntaggedEnums {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
        if item.span.from_expansion() || !matches!(item.kind, ItemKind::Enum(..)) {
            return;
        }
        let Some(source) = AuthoredItemSource::for_item(cx, item) else {
            return;
        };

        let Ok(enumeration) = syn::parse_str::<syn::ItemEnum>(&source) else {
            return;
        };
        let container = SerdeAttributes::from_attributes(&enumeration.attrs);
        if !container.has(SerdeFlag::Untagged) {
            return;
        }

        let variants = enumeration
            .variants
            .iter()
            .filter_map(|variant| {
                VariantShape::from_variant(variant).map(|mut shape| {
                    shape.is_closed = container.has(SerdeFlag::DenyUnknownFields);
                    shape
                })
            })
            .collect::<Vec<_>>();

        for (index, first) in variants.iter().enumerate() {
            for second in &variants[index + 1..] {
                let Some(witness) = first.overlap_witness(second) else {
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
