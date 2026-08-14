extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::BTreeSet;

use rustc_errors::DiagDecorator;
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use super::utils::contracts::{SerdeAttributes, SerdeContractCatalog, SerdeFlag};
use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::AuthoredItemSource;

// -----------------------------------------------------------------------------
// Violation: Remote representation drifting from its source
// -----------------------------------------------------------------------------

/// Current field schema of a type represented through Serde's remote mechanism.
struct SourceSchema {
    /// Compiler-qualified path used to distinguish same-named local types.
    path: String,
    /// Simple declaration name used for module-local resolution.
    name: String,
    /// Module containing the source declaration.
    scope: String,
    /// Authored fields relevant to the contract.
    fields: BTreeSet<String>,
}

/// Authored remote representation awaiting comparison with its source type.
struct RemoteCandidate {
    /// Local declaration identity used to associate evidence collected in separate passes.
    definition: LocalDefId,
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Authored source used to recover framework metadata.
    source: String,
    /// Module path in which the remote string is resolved.
    scope: String,
    /// Authored fields relevant to the contract.
    serialize_fields: BTreeSet<String>,
    /// Fields represented during deserialization.
    deserialize_fields: BTreeSet<String>,
}

/// Remote representation missing fields from its current source type.
struct Violation {
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Authored source used to recover framework metadata.
    source: String,
    /// Source fields absent from the remote representation.
    missing: Vec<String>,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "Serde remote representation drifts from `{}`",
            self.source
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "the undocumented remote projection omits visible source {} {}",
            if self.missing.len() == 1 {
                "field"
            } else {
                "fields"
            },
            self.missing.join(", ")
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "mirror the missing fields, or document this as a versioned projection and its reconstruction policy",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            SERDE_REMOTE_REPRESENTATIONS_DRIFTING_FROM_SOURCES,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.span,
                    "this remote schema is an implicit subset of its source",
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// SerdeRemoteRepresentationsDriftingFromSources: Synchronized schema policy
// -----------------------------------------------------------------------------

/// Correlates remote Serde representations with their current source declarations.
#[derive(Default)]
struct SerdeRemoteRepresentationsDriftingFromSources {
    /// Effective Serde contracts consulted after all local declarations are known.
    catalog: SerdeContractCatalog,
    /// Source schemas keyed by the type path used in `remote` attributes.
    sources: Vec<SourceSchema>,
    /// Authored Serde declarations awaiting crate-wide contract comparison.
    candidates: Vec<RemoteCandidate>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub SERDE_REMOTE_REPRESENTATIONS_DRIFTING_FROM_SOURCES,
    Warn,
    "finds undocumented drift in local Serde remote representations",
    SerdeRemoteRepresentationsDriftingFromSources::default()
}

impl LateLintPass<'_> for SerdeRemoteRepresentationsDriftingFromSources {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
        if item.span.from_expansion() || !matches!(item.kind, ItemKind::Struct(..)) {
            return;
        }
        let Some(source) = AuthoredItemSource::for_item(cx, item) else {
            return;
        };

        let Ok(structure) = syn::parse_str::<syn::ItemStruct>(&source) else {
            return;
        };

        let fields = structure
            .fields
            .iter()
            .filter_map(|field| field.ident.as_ref().map(ToString::to_string))
            .collect::<BTreeSet<_>>();
        let attributes = SerdeAttributes::from_attributes(&structure.attrs);

        let Some(remote) = attributes.remote else {
            let path = cx.tcx.def_path_str(item.owner_id.to_def_id());
            self.sources.push(SourceSchema {
                scope: path
                    .rsplit_once("::")
                    .map_or_else(String::new, |(scope, _)| scope.to_owned()),
                name: structure.ident.to_string(),
                path,
                fields,
            });
            return;
        };

        let documentation = structure
            .attrs
            .iter()
            .filter(|attribute| attribute.path().is_ident("doc"))
            .filter_map(|attribute| attribute.meta.require_name_value().ok())
            .filter_map(|value| match &value.value {
                syn::Expr::Lit(expression) => match &expression.lit {
                    syn::Lit::Str(value) => Some(value.value().to_ascii_lowercase()),
                    _ => None,
                },
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(" ");
        if ["projection", "versioned"]
            .iter()
            .any(|term| documentation.contains(term))
            && ["omit", "reconstruct", "synthesi"]
                .iter()
                .any(|term| documentation.contains(term))
        {
            return;
        }

        let mut serialize_fields = BTreeSet::new();
        let mut deserialize_fields = BTreeSet::new();
        for field in &structure.fields {
            let Some(name) = field.ident.as_ref().map(ToString::to_string) else {
                continue;
            };
            let attributes = SerdeAttributes::from_attributes(&field.attrs);
            if !attributes.has(SerdeFlag::SkipSerialize) {
                serialize_fields.insert(name.clone());
            }
            if !attributes.has(SerdeFlag::SkipDeserialize) {
                deserialize_fields.insert(name);
            }
        }

        self.candidates.push(RemoteCandidate {
            definition: item.owner_id.def_id,
            span: item.span,
            source: remote,
            scope: cx
                .tcx
                .def_path_str(item.owner_id.to_def_id())
                .rsplit_once("::")
                .map_or_else(String::new, |(scope, _)| scope.to_owned()),
            serialize_fields,
            deserialize_fields,
        });
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for candidate in self.candidates.drain(..) {
            if self
                .catalog
                .derived_type(candidate.definition, "Serialize")
                .is_none()
                && self
                    .catalog
                    .derived_type(candidate.definition, "Deserialize")
                    .is_none()
            {
                continue;
            }

            let relative = candidate
                .source
                .strip_prefix("self::")
                .unwrap_or(&candidate.source);
            let expected = if candidate.source.starts_with("crate::") {
                candidate.source.clone()
            } else {
                format!("{}::{relative}", candidate.scope)
            };
            let mut matches = self
                .sources
                .iter()
                .filter(|source| {
                    source.path == expected
                        || (!candidate.source.contains("::")
                            && source.scope == candidate.scope
                            && source.name == candidate.source)
                })
                .collect::<Vec<_>>();
            if matches.is_empty() {
                matches = self
                    .sources
                    .iter()
                    .filter(|source| {
                        source.path == candidate.source
                            || source.path.ends_with(&format!("::{}", candidate.source))
                    })
                    .collect();
            }
            let [source] = matches.as_slice() else {
                continue;
            };

            let serializes = self
                .catalog
                .derived_type(candidate.definition, "Serialize")
                .is_some();
            let deserializes = self
                .catalog
                .derived_type(candidate.definition, "Deserialize")
                .is_some();
            let missing = source
                .fields
                .iter()
                .filter(|field| {
                    (serializes && !candidate.serialize_fields.contains(*field))
                        || (deserializes && !candidate.deserialize_fields.contains(*field))
                })
                .map(|name| format!("`{name}`"))
                .collect::<Vec<_>>();

            if missing.is_empty() {
                continue;
            }

            Violation {
                span: candidate.span,
                source: candidate.source,
                missing,
            }
            .emit(cx);
        }
    }
}
