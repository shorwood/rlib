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
        // Only named-field variants expose distinguishable object-key contracts.
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

            // Fields skipped during deserialization do not constrain accepted input shapes.
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

            // Defaulted and optional fields are not required members of the witness shape.
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

        // Closed variants reject a union witness containing any unknown required field.
        if (self.is_closed && !required.is_subset(&self.fields.keys().cloned().collect()))
            || (second.is_closed && !required.is_subset(&second.fields.keys().cloned().collect()))
        {
            return None;
        }

        for key in &required {
            // A shared required field with disjoint scalar domains prevents overlap.
            if let (Some(first_domain), Some(second_domain)) =
                (self.fields.get(key), second.fields.get(key))
                && !SerdeAmbiguousUntaggedEnums::domains_overlap(first_domain, second_domain)
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

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub SERDE_AMBIGUOUS_UNTAGGED_ENUMS,
    Warn,
    "finds overlapping Serde untagged enum variants",
    SerdeAmbiguousUntaggedEnums::default()
}

impl SerdeAmbiguousUntaggedEnums {
    /// Returns whether two scalar domains share at least one wire value.
    fn domains_overlap(first: &str, second: &str) -> bool {
        let numeric = |domain: &str| {
            matches!(
                domain,
                "i8" | "i16"
                    | "i32"
                    | "i64"
                    | "i128"
                    | "isize"
                    | "u8"
                    | "u16"
                    | "u32"
                    | "u64"
                    | "u128"
                    | "usize"
                    | "f32"
                    | "f64"
            )
        };
        first == second
            || (numeric(first) && numeric(second))
            || matches!((first, second), ("char", "string") | ("string", "char"))
    }

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
        // Only path types can name the supported primitive or optional domains.
        let syn::Type::Path(path) = ty else {
            return None;
        };
        let segment = path.path.segments.last()?;

        let name = segment.ident.to_string();

        // Primitive paths can be classified without container unwrapping.
        if name != "Option" {
            return Self::primitive_domain(&name);
        }

        // Unwrap an optional scalar while preserving its overlap domain.
        // Optional syntax requires an angle-bracketed inner type argument.
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

        // Retain only authored enum declarations for variant-shape analysis.
        if item.span.from_expansion() || !matches!(item.kind, ItemKind::Enum(..)) {
            return;
        }

        // Missing authored source prevents Serde attribute and field-shape recovery.
        let Some(source) = AuthoredItemSource::for_item(cx, item) else {
            return;
        };

        // Unparseable enum text cannot provide reliable variant contracts.
        let Ok(enumeration) = syn::parse_str::<syn::ItemEnum>(&source) else {
            return;
        };
        let container = SerdeAttributes::from_attributes(&enumeration.attrs);

        // Tagged enums already carry an explicit disambiguating wire field.
        if !container.has(SerdeFlag::Untagged) {
            return;
        }

        let variants = enumeration
            .variants
            .iter()
            .filter_map(|variant| {
                // Variants skipped during deserialization accept no input shape.
                if SerdeAttributes::from_attributes(&variant.attrs).has(SerdeFlag::SkipDeserialize)
                {
                    return None;
                }
                VariantShape::from_variant(variant).map(|mut shape| {
                    shape.is_closed = container.has(SerdeFlag::DenyUnknownFields);
                    shape
                })
            })
            .collect::<Vec<_>>();

        for (index, first) in variants.iter().enumerate() {
            for second in &variants[index + 1..] {
                // Disjoint variant shapes provide no ambiguous witness.
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
            // Without generated deserialization, the authored shapes are not active wire inputs.
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
