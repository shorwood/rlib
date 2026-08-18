extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use super::utils::contracts::{SerdeAttributes, SerdeContractCatalog, SerdeFlag};
use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::AuthoredItemSource;

// -----------------------------------------------------------------------------
// Violation: Asymmetric serialization contract
// -----------------------------------------------------------------------------

/// Public Serde type awaiting comparison of its two wire directions.
struct Candidate {
    /// Local declaration identity used to associate evidence collected in separate passes.
    definition: LocalDefId,
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Qualified declaration name shown in the diagnostic.
    declaration: String,
    /// Effective name written during serialization.
    serialize: String,
    /// Primary name accepted during deserialization.
    deserialize: String,
}

/// Direction-specific wire names extracted from one Serde declaration.
struct DirectionalNames {
    /// Name emitted during serialization.
    serialize: String,
    /// Name accepted during deserialization.
    deserialize: String,
}

impl DirectionalNames {
    /// Extracts unequal directional names that lack an alias or compatibility explanation.
    fn from_attributes(attributes: &[syn::Attribute]) -> Option<Self> {
        // Authored compatibility documentation makes directional naming intentional policy.
        if has_compatibility_explanation(attributes) {
            return None;
        }
        let attributes = SerdeAttributes::from_attributes(attributes);

        // One-way skipped declarations do not promise a symmetric round-trip contract.
        if attributes.has(SerdeFlag::SkipSerialize) || attributes.has(SerdeFlag::SkipDeserialize) {
            return None;
        }

        // Comparing directions requires explicit names for both wire operations.
        let (Some(serialize), Some(deserialize)) =
            (attributes.rename_serialize, attributes.rename_deserialize)
        else {
            return None;
        };
        (serialize != deserialize && !attributes.aliases.contains(&serialize)).then_some(Self {
            serialize,
            deserialize,
        })
    }
}

/// Public type with incompatible serialization and deserialization contracts.
struct Violation {
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Qualified declaration name shown in the diagnostic.
    declaration: String,
    /// Effective name written during serialization.
    serialize: String,
    /// Primary name accepted during deserialization.
    deserialize: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "Serde names for `{}` differ by direction",
            self.declaration
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "serialization writes `{}` while deserialization accepts `{}` as the primary name, without an authored compatibility explanation",
            self.serialize, self.deserialize
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "use one stable name or document the intentional one-way schema migration at this declaration",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            SERDE_ASYMMETRIC_SERDE_CONTRACTS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this declaration has directional wire names");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// SerdeAsymmetricSerdeContracts: Symmetric wire-contract policy
// -----------------------------------------------------------------------------

/// Finds public types whose serialization and deserialization contracts disagree.
#[derive(Default)]
struct SerdeAsymmetricSerdeContracts {
    /// Effective Serde contracts consulted after all local declarations are known.
    catalog: SerdeContractCatalog,
    /// Authored Serde declarations awaiting crate-wide contract comparison.
    candidates: Vec<Candidate>,
}

impl LateLintPass<'_> for SerdeAsymmetricSerdeContracts {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);

        // Expanded items provide derive evidence rather than authored naming policy.
        if item.span.from_expansion() {
            return;
        }

        // Missing authored source prevents declaration-level attribute recovery.
        let Some(source) = AuthoredItemSource::for_item(cx, item) else {
            return;
        };

        // Recover enum members outside the declaration-kind dispatch.
        let enum_members = || {
            let enumeration = match syn::parse_str::<syn::ItemEnum>(&source) {
                Ok(enumeration) => enumeration,
                // Unparseable enum text cannot provide reliable member attributes.
                Err(_error) => return None,
            };
            let mut members = vec![(enumeration.ident.to_string(), enumeration.attrs)];
            for variant in enumeration.variants {
                let variant_name = variant.ident.to_string();
                members.push((variant_name.clone(), variant.attrs));
                members.extend(variant.fields.iter().enumerate().map(|(index, field)| {
                    let field_name = field
                        .ident
                        .as_ref()
                        .map_or_else(|| index.to_string(), ToString::to_string);
                    (format!("{variant_name}.{field_name}"), field.attrs.clone())
                }));
            }
            Some(members)
        };

        let members = match item.kind {
            ItemKind::Struct(..) => {
                // Unparseable struct text cannot provide reliable member attributes.
                let Ok(structure) = syn::parse_str::<syn::ItemStruct>(&source) else {
                    return;
                };

                let mut members = vec![(structure.ident.to_string(), structure.attrs)];
                members.extend(structure.fields.iter().enumerate().map(|(index, field)| {
                    (
                        field
                            .ident
                            .as_ref()
                            .map_or_else(|| format!("field {index}"), ToString::to_string),
                        field.attrs.clone(),
                    )
                }));
                members
            }
            ItemKind::Enum(..) => {
                // Invalid enum source cannot contribute directional member contracts.
                let Some(members) = enum_members() else {
                    return;
                };
                members
            }
            // Other declarations do not expose Serde struct or enum member contracts.
            _ => return,
        };

        for (declaration, attributes) in members {
            let Some(DirectionalNames {
                serialize,
                deserialize,
            }) = DirectionalNames::from_attributes(&attributes)
            else {
                // Symmetric or explicitly compatible members need no crate-wide comparison.
                continue;
            };

            self.candidates.push(Candidate {
                definition: item.owner_id.def_id,
                span: item.span,
                declaration,
                serialize,
                deserialize,
            });
        }
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

            // Both directions must be generated before their names form one round-trip contract.
            if !serializes || !deserializes {
                continue;
            }

            Violation {
                span: candidate.span,
                declaration: candidate.declaration,
                serialize: candidate.serialize,
                deserialize: candidate.deserialize,
            }
            .emit(cx);
        }
    }
}

/// Returns whether documentation explains an intentionally directional wire contract.
fn has_compatibility_explanation(attributes: &[syn::Attribute]) -> bool {
    let documentation = attributes
        .iter()
        .filter(|attribute| attribute.path().is_ident("doc"))
        .filter_map(|attribute| {
            // Non-name-value documentation forms provide no literal prose to classify.
            let Ok(value) = attribute.meta.require_name_value() else {
                return None;
            };
            Some(value)
        })
        .filter_map(|value| match &value.value {
            syn::Expr::Lit(expression) => match &expression.lit {
                syn::Lit::Str(value) => Some(value.value().to_ascii_lowercase()),
                _ => None,
            },
            _ => None,
        })
        .collect::<Vec<_>>();
    let documentation = documentation.join(" ");

    let explains_compatibility = ["compatib", "legacy", "migrat", "backward"]
        .iter()
        .any(|term| documentation.contains(term));
    let explains_direction = ["accept", "deserial", "read", "serial", "write"]
        .iter()
        .any(|term| documentation.contains(term));
    explains_compatibility && explains_direction
}

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub SERDE_ASYMMETRIC_SERDE_CONTRACTS,
    Warn,
    "finds unexplained directional Serde naming contracts",
    SerdeAsymmetricSerdeContracts::default()
}
