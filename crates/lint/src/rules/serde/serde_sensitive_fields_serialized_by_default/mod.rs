extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

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
    /// Stores the `fields` value used by this analysis.
    fields: Vec<String>,
}

/// Carries the `Violation` state used by this analysis.
struct Violation {
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `fields` value used by this analysis.
    fields: Vec<String>,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("Serde serializes likely credentials by default")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "{} {} strong credential vocabulary with raw string or byte storage and {} no skip, redaction, or encryption policy",
            self.fields.join(", "),
            if self.fields.len() == 1 {
                "combines"
            } else {
                "combine"
            },
            if self.fields.len() == 1 {
                "has"
            } else {
                "have"
            }
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "skip these fields, use a redacting or encrypting serializer, or introduce a secret-aware wrapper",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            SERDE_SENSITIVE_FIELDS_SERIALIZED_BY_DEFAULT,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.span,
                    "this public serialization contract exposes raw sensitive data",
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

/// Performs the `sensitive_name` step of the lint analysis.
fn sensitive_name(name: &str) -> bool {
    // Prepare the values used by this stage.
    let name = name.to_ascii_lowercase();

    // Perform the next step of the analysis.
    [
        "password",
        "passphrase",
        "access_token",
        "refresh_token",
        "auth_token",
        "api_token",
        "api_key",
        "private_key",
        "client_secret",
        "shared_secret",
    ]
    .iter()
    .any(|term| name == *term || name.ends_with(&format!("_{term}")))
}

/// Performs the `is_u8` step of the lint analysis.
fn is_u8(ty: &syn::Type) -> bool {
    matches!(ty, syn::Type::Path(path)
        if path.path.segments.last().is_some_and(|segment| segment.ident == "u8"))
}

/// Performs the `raw_secret_carrier` step of the lint analysis.
fn raw_secret_carrier(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Array(array) => is_u8(&array.elem),
        syn::Type::Slice(slice) => is_u8(&slice.elem),
        syn::Type::Path(path) => {
            // Prepare the values used by this stage.
            let Some(segment) = path.path.segments.last() else {
                return false;
            };

            // Classify the current analyze_candidate.
            match segment.ident.to_string().as_str() {
                "String" => true,
                "Vec" => match &segment.arguments {
                    syn::PathArguments::AngleBracketed(arguments) => arguments.args.iter().any(
                        |argument| matches!(argument, syn::GenericArgument::Type(ty) if is_u8(ty)),
                    ),
                    _ => false,
                },
                _ => false,
            }
        }
        _ => false,
    }
}

/// Performs the `explicit_sensitive_policy` step of the lint analysis.
fn explicit_sensitive_policy(path: &str) -> bool {
    let path = path.to_ascii_lowercase();
    ["redact", "secret", "encrypt", "mask"]
        .iter()
        .any(|term| path.contains(term))
}

#[derive(Default)]
/// Carries the `SerdeSensitiveFieldsSerializedByDefault` state used by this analysis.
struct SerdeSensitiveFieldsSerializedByDefault {
    /// Stores the `catalog` value used by this analysis.
    catalog: SerdeContractCatalog,
    /// Stores the `candidates` value used by this analysis.
    candidates: Vec<Candidate>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub SERDE_SENSITIVE_FIELDS_SERIALIZED_BY_DEFAULT,
    Warn,
    "finds raw credential fields included in public Serde serializers",
    SerdeSensitiveFieldsSerializedByDefault::default()
}

impl LateLintPass<'_> for SerdeSensitiveFieldsSerializedByDefault {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Update the accumulated analysis state.
        self.catalog.check_item(cx, item);
        if item.span.from_expansion() || !matches!(item.kind, ItemKind::Struct(..)) {
            return;
        }
        let Some(source) = AuthoredItemSource::for_item(cx, item) else {
            return;
        };

        // Prepare the values used by this stage.
        let Ok(structure) = syn::parse_str::<syn::ItemStruct>(&source) else {
            return;
        };
        if !matches!(structure.vis, syn::Visibility::Public(_)) {
            return;
        }

        // Prepare the values used by this stage.
        let fields = structure
            .fields
            .iter()
            .filter_map(|field| {
                let name = field.ident.as_ref()?.to_string();
                let attributes = SerdeAttributes::analyze_serde_attributes(&field.attrs);
                if attributes.has(SerdeFlag::SkipSerialize)
                    || !sensitive_name(&name)
                    || !raw_secret_carrier(&field.ty)
                    || attributes
                        .serialize_with
                        .as_deref()
                        .is_some_and(explicit_sensitive_policy)
                {
                    return None;
                }
                Some(format!("`{name}`"))
            })
            .collect::<Vec<_>>();

        // Reject inputs that do not satisfy this stage.
        if fields.is_empty() {
            return;
        }

        // Update the accumulated analysis state.
        self.candidates.push(Candidate {
            definition: item.owner_id.def_id,
            span: item.span,
            fields,
        });
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for analyze_candidate in self.candidates.drain(..) {
            // Reject inputs that do not satisfy this stage.
            if self
                .catalog
                .derived_type(analyze_candidate.definition, "Serialize")
                .is_none()
            {
                continue;
            }

            // Perform the next step of the analysis.
            Violation {
                span: analyze_candidate.span,
                fields: analyze_candidate.fields,
            }
            .emit(cx);
        }
    }
}
