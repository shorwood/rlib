extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty::{self, Ty};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use super::utils::contracts::{
    SerdeAttributes, SerdeAuthoredField, SerdeAuthoredFieldSet, SerdeContractCatalog, SerdeFlag,
};
use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::AuthoredItemSource;

// -----------------------------------------------------------------------------
// Violation: Sensitive field serialized by default
// -----------------------------------------------------------------------------

/// Sensitive-looking field awaiting carrier-type and explicit-policy checks.
struct Candidate {
    /// Local declaration identity used to associate evidence collected in separate passes.
    definition: LocalDefId,
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Authored fields relevant to the contract.
    fields: Vec<String>,
}

/// Raw secret-bearing field included unchanged in serialized output.
struct Violation {
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Authored fields relevant to the contract.
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

// -----------------------------------------------------------------------------
// SerdeSensitiveFieldsSerializedByDefault: Explicit disclosure policy
// -----------------------------------------------------------------------------

/// Rejects sensitive-looking fields that Serde would expose unchanged by default.
#[derive(Default)]
struct SerdeSensitiveFieldsSerializedByDefault {
    /// Effective Serde contracts consulted after all local declarations are known.
    catalog: SerdeContractCatalog,
    /// Authored Serde declarations awaiting crate-wide contract comparison.
    candidates: Vec<Candidate>,
}

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub SERDE_SENSITIVE_FIELDS_SERIALIZED_BY_DEFAULT,
    Warn,
    "finds raw credential fields included in public Serde serializers",
    SerdeSensitiveFieldsSerializedByDefault::default()
}

impl SerdeSensitiveFieldsSerializedByDefault {
    /// Recognizes field names conventionally associated with credentials or secret material.
    fn sensitive_name(name: &str) -> bool {
        let name = name.strip_prefix("r#").unwrap_or(name).to_ascii_lowercase();
        let components = name.split(['_', '.']).collect::<Vec<_>>();

        // Names that explicitly describe protection should not be treated as raw secrets.
        if components.iter().any(|component| {
            matches!(
                *component,
                "encrypted" | "hashed" | "masked" | "redacted" | "sanitized" | "scrubbed"
            )
        }) {
            return false;
        }

        [
            &["password"][..],
            &["passphrase"],
            &["access", "token"],
            &["refresh", "token"],
            &["auth", "token"],
            &["api", "token"],
            &["api", "key"],
            &["private", "key"],
            &["client", "secret"],
            &["shared", "secret"],
        ]
        .iter()
        .any(|term| components.windows(term.len()).any(|window| window == *term))
    }

    /// Recognizes strings and byte containers that can expose raw secret material.
    fn raw_secret_carrier(cx: &LateContext<'_>, ty: Ty<'_>) -> bool {
        match ty.kind() {
            ty::Str => true,
            ty::Ref(_, inner, _) => Self::raw_secret_carrier(cx, *inner),
            ty::Slice(inner) | ty::Array(inner, _) => {
                matches!(inner.kind(), ty::Uint(ty::UintTy::U8))
            }
            ty::Adt(definition, arguments) => {
                let path = cx.tcx.def_path_str(definition.did());
                path.ends_with("::string::String")
                    || (path.ends_with("::vec::Vec")
                        && arguments.types().next().is_some_and(|element| {
                            matches!(element.kind(), ty::Uint(ty::UintTy::U8))
                        }))
            }
            _ => false,
        }
    }

    /// Returns whether Serde attributes explicitly omit or transform a sensitive field.
    fn explicit_sensitive_policy(path: &str) -> bool {
        let path = path.to_ascii_lowercase();
        ["redact", "secret", "encrypt", "mask"]
            .iter()
            .any(|term| path.contains(term))
    }
}

impl LateLintPass<'_> for SerdeSensitiveFieldsSerializedByDefault {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);

        // Generated and non-data items cannot expose authored serializable fields.
        if item.span.from_expansion()
            || !matches!(item.kind, ItemKind::Struct(..) | ItemKind::Enum(..))
        {
            return;
        }

        // Missing authored source prevents reliable Serde attribute recovery.
        let Some(source) = AuthoredItemSource::for_item(cx, item) else {
            return;
        };

        // Items without an authored field model expose no field-level Serde contract here.
        let Some(SerdeAuthoredFieldSet {
            is_public: public,
            fields,
            ..
        }) = SerdeAuthoredFieldSet::for_item(item, &source)
        else {
            return;
        };

        // Private data types are outside this public secret-exposure policy.
        if !public {
            return;
        }

        let fields = fields
            .into_iter()
            .filter_map(|field| {
                let SerdeAuthoredField {
                    name,
                    attributes: authored_attributes,
                    definition: field_definition,
                } = field;
                let attributes = SerdeAttributes::from_attributes(&authored_attributes);
                let is_raw_default = !attributes.has(SerdeFlag::SkipSerialize)
                    && Self::sensitive_name(&name)
                    && Self::raw_secret_carrier(
                        cx,
                        cx.tcx.type_of(field_definition).instantiate_identity(),
                    )
                    && !attributes
                        .serialize_with
                        .as_deref()
                        .is_some_and(Self::explicit_sensitive_policy);
                is_raw_default.then(|| format!("`{name}`"))
            })
            .collect::<Vec<_>>();

        // A public type without raw sensitive fields has no exposure to report.
        if fields.is_empty() {
            return;
        }

        self.candidates.push(Candidate {
            definition: item.owner_id.def_id,
            span: item.span,
            fields,
        });
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for candidate in self.candidates.drain(..) {
            if !cx
                .tcx
                .effective_visibilities(())
                .is_exported(candidate.definition)
            {
                continue;
            }
            if self
                .catalog
                .derived_type(candidate.definition, "Serialize")
                .is_none()
            {
                continue;
            }

            Violation {
                span: candidate.span,
                fields: candidate.fields,
            }
            .emit(cx);
        }
    }
}
