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

use super::contracts::{SerdeAttributes, SerdeContractCatalog, SerdeFlag};
use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::AuthoredItemSource;

/// Carries the `Candidate` state used by this analysis.
struct Candidate {
    /// Stores the `definition` value used by this analysis.
    definition: LocalDefId,
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `first` value used by this analysis.
    first: String,
    /// Stores the `second` value used by this analysis.
    second: String,
    /// Stores the `witness` value used by this analysis.
    witness: Vec<String>,
}

/// Carries the `Violation` state used by this analysis.
struct Violation {
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `first` value used by this analysis.
    first: String,
    /// Stores the `second` value used by this analysis.
    second: String,
    /// Stores the `witness` value used by this analysis.
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

/// Carries the `VariantShape` state used by this analysis.
struct VariantShape {
    /// Stores the `name` value used by this analysis.
    name: String,
    /// Stores the `fields` value used by this analysis.
    fields: BTreeMap<String, &'static str>,
    /// Stores the `required` value used by this analysis.
    required: BTreeSet<String>,
    /// Stores the `is_closed` value used by this analysis.
    is_closed: bool,
}

impl VariantShape {
    /// Performs the `analyze_variant_shape` step of the lint analysis.
    fn analyze_variant_shape(variant: &syn::Variant) -> Option<Self> {
        // Prepare the values used by this stage.
        let syn::Fields::Named(fields) = &variant.fields else {
            return None;
        };

        // Prepare the values used by this stage.
        let mut shape = Self {
            name: variant.ident.to_string(),
            fields: BTreeMap::new(),
            required: BTreeSet::new(),
            is_closed: false,
        };

        // Process the candidates handled by this stage.
        for field in &fields.named {
            // Prepare the values used by this stage.
            let attributes = SerdeAttributes::analyze_serde_attributes(&field.attrs);
            if attributes.has(SerdeFlag::SkipDeserialize) {
                continue;
            }
            let rust_name = field.ident.as_ref()?.to_string();
            let has_default = attributes.has(SerdeFlag::HasDefault);
            let name = attributes.rename_deserialize.unwrap_or(rust_name);

            // Perform the next step of the analysis.
            shape.fields.insert(name.clone(), scalar_domain(&field.ty)?);
            if !(!has_default && !is_option(&field.ty)) {
                continue;
            }
            shape.required.insert(name);
        }

        // Return the completed analysis result.
        Some(shape)
    }

    /// Returns a concrete field set accepted by both variant shapes.
    fn overlap_witness(&self, second: &Self) -> Option<Vec<String>> {
        // Prepare the values used by this stage.
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

        // Process the candidates handled by this stage.
        for key in &required {
            if let (Some(first_domain), Some(second_domain)) =
                (self.fields.get(key), second.fields.get(key))
                && first_domain != second_domain
            {
                return None;
            }
        }

        // Return the completed analysis result.
        Some(required.into_iter().collect())
    }
}

/// Recognizes signed integer scalar domains.
fn signed_scalar_domain(name: &str) -> Option<&'static str> {
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
fn unsigned_scalar_domain(name: &str) -> Option<&'static str> {
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
fn float_scalar_domain(name: &str) -> Option<&'static str> {
    match name {
        "f32" => Some("f32"),
        "f64" => Some("f64"),
        _ => None,
    }
}

/// Classifies primitive scalar names into their overlap domains.
fn primitive_scalar_domain(name: &str) -> Option<&'static str> {
    match name {
        "bool" => Some("bool"),
        "char" => Some("char"),
        "String" | "str" => Some("string"),
        _ => signed_scalar_domain(name)
            .or_else(|| unsigned_scalar_domain(name))
            .or_else(|| float_scalar_domain(name)),
    }
}

/// Performs the `scalar_domain` step of the lint analysis.
fn scalar_domain(ty: &syn::Type) -> Option<&'static str> {
    // Prepare the values used by this stage.
    let syn::Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;

    let name = segment.ident.to_string();
    if name != "Option" {
        return primitive_scalar_domain(&name);
    }

    // Unwrap an optional scalar while preserving its overlap domain.
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    arguments.args.iter().find_map(|arg| {
        if let syn::GenericArgument::Type(inner) = arg {
            scalar_domain(inner)
        } else {
            None
        }
    })
}

/// Performs the `is_option` step of the lint analysis.
fn is_option(ty: &syn::Type) -> bool {
    matches!(ty, syn::Type::Path(path)
        if path.path.segments.last().is_some_and(|segment| segment.ident == "Option"))
}

#[derive(Default)]
/// Carries the `SerdeAmbiguousUntaggedEnums` state used by this analysis.
struct SerdeAmbiguousUntaggedEnums {
    /// Stores the `catalog` value used by this analysis.
    catalog: SerdeContractCatalog,
    /// Stores the `candidates` value used by this analysis.
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
        // Update the accumulated analysis state.
        self.catalog.check_item(cx, item);
        if item.span.from_expansion() || !matches!(item.kind, ItemKind::Enum(..)) {
            return;
        }
        let Some(source) = AuthoredItemSource::for_item(cx, item) else {
            return;
        };

        // Prepare the values used by this stage.
        let Ok(enumeration) = syn::parse_str::<syn::ItemEnum>(&source) else {
            return;
        };
        let container = SerdeAttributes::analyze_serde_attributes(&enumeration.attrs);
        if !container.has(SerdeFlag::Untagged) {
            return;
        }

        // Prepare the values used by this stage.
        let variants = enumeration
            .variants
            .iter()
            .filter_map(|variant| {
                VariantShape::analyze_variant_shape(variant).map(|mut shape| {
                    shape.is_closed = container.has(SerdeFlag::DenyUnknownFields);
                    shape
                })
            })
            .collect::<Vec<_>>();

        // Process the candidates handled by this stage.
        for (index, first) in variants.iter().enumerate() {
            for second in &variants[index + 1..] {
                // Prepare the values used by this stage.
                let Some(witness) = first.overlap_witness(second) else {
                    continue;
                };

                // Update the accumulated analysis state.
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
        for analyze_candidate in self.candidates.drain(..) {
            // Reject inputs that do not satisfy this stage.
            if self
                .catalog
                .derived_type(analyze_candidate.definition, "Deserialize")
                .is_none()
            {
                continue;
            }

            // Perform the next step of the analysis.
            Violation {
                span: analyze_candidate.span,
                first: analyze_candidate.first,
                second: analyze_candidate.second,
                witness: analyze_candidate
                    .witness
                    .into_iter()
                    .map(|key| format!("`{key}`"))
                    .collect(),
            }
            .emit(cx);
        }
    }
}
