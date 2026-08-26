extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::HashSet;

use rustc_errors::{Applicability, DiagDecorator};
use rustc_hir::{EnumDef, HirId, Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use super::utils::contracts::{
    SerdeAttributes, SerdeCase, SerdeContractCatalog, SerdeDirection, SerdeFlag,
};
use crate::utils::diagnostic::LateViolation;
use crate::utils::name_policy::{CandidatePolicy, factor_names, standalone_attribute_span};
use crate::utils::source_provenance::AuthoredItemSource;

const CASES: [&str; 8] = [
    "lowercase",
    "UPPERCASE",
    "PascalCase",
    "camelCase",
    "snake_case",
    "SCREAMING_SNAKE_CASE",
    "kebab-case",
    "SCREAMING-KEBAB-CASE",
];

struct Candidate {
    definition: LocalDefId,
    owner: HirId,
    span: Span,
    activation: Activation,
    detail: String,
    remediation: String,
    suggestion: Option<(Span, String)>,
    private_only: bool,
}

#[derive(Clone, Copy)]
enum Activation {
    Direction(SerdeDirection),
    Both,
    Only(SerdeDirection),
}

impl LateViolation for Candidate {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("this Serde rename policy is noncanonical")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.detail)
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.remediation)
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            SERDE_NONCANONICAL_RENAME_POLICIES,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.note(self.rationale_message().into_owned());
                let remediation = self.remediation_message().into_owned();
                if let Some((span, replacement)) = self.suggestion {
                    diag.span_suggestion(
                        span,
                        remediation,
                        replacement,
                        Applicability::MachineApplicable,
                    );
                } else {
                    diag.help(remediation);
                }
            }),
        );
    }
}

struct Member {
    name: String,
    attributes: SerdeAttributes,
    is_variant: bool,
}

impl Member {
    fn rust_name(&self) -> &str {
        self.name.strip_prefix("r#").unwrap_or(&self.name)
    }

    fn inherited_name(&self, rule: Option<&str>) -> String {
        if self.is_variant {
            SerdeCase::apply_to_variant(self.rust_name(), rule)
        } else {
            SerdeCase::apply_to_field(self.rust_name(), rule)
        }
    }
}

struct Group<'a> {
    definition: LocalDefId,
    owner: HirId,
    span: Span,
    scope: &'a str,
    inherited: [Option<&'a str>; 2],
    members: &'a [Member],
}

#[derive(Clone, Copy)]
struct DirectionalNames<'a> {
    serialize: Option<&'a str>,
    deserialize: Option<&'a str>,
    authored_directionally: bool,
    subject: &'a str,
}

struct Removal {
    expected: String,
    detail: String,
    remediation: &'static str,
    private_only: bool,
}

#[derive(Default)]
struct SerdeNoncanonicalRenamePolicies {
    catalog: SerdeContractCatalog,
    candidates: Vec<Candidate>,
}

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub SERDE_NONCANONICAL_RENAME_POLICIES,
    Warn,
    "finds Serde member names that should inherit a container naming policy",
    SerdeNoncanonicalRenamePolicies::default()
}

impl SerdeNoncanonicalRenamePolicies {
    fn explicit_name(member: &Member, direction: SerdeDirection) -> Option<&str> {
        match direction {
            SerdeDirection::Serialize => member.attributes.rename_serialize.as_deref(),
            SerdeDirection::Deserialize => member.attributes.rename_deserialize.as_deref(),
        }
    }

    fn record_directional_names(&mut self, group: &Group<'_>, names: DirectionalNames<'_>) {
        if !names.authored_directionally {
            return;
        }
        if names.serialize.is_some() && names.serialize == names.deserialize {
            self.candidates.push(Candidate {
                definition: group.definition,
                owner: group.owner,
                span: group.span,
                activation: Activation::Both,
                detail: format!(
                    "{} declares identical serialization and deserialization names",
                    names.subject
                ),
                remediation: "use the nondirectional Serde shorthand for the shared name"
                    .to_owned(),
                suggestion: None,
                private_only: false,
            });
            return;
        }
        for (active, inactive_name, inactive_label) in [
            (
                SerdeDirection::Serialize,
                names.deserialize,
                "deserialization",
            ),
            (
                SerdeDirection::Deserialize,
                names.serialize,
                "serialization",
            ),
        ] {
            if inactive_name.is_none() {
                continue;
            }
            self.candidates.push(Candidate {
                definition: group.definition,
                owner: group.owner,
                span: group.span,
                activation: Activation::Only(active),
                detail: format!(
                    "{} configures {inactive_label}, but this private type derives only {}",
                    names.subject,
                    active.label()
                ),
                remediation: format!("remove the inactive {inactive_label} naming branch"),
                suggestion: None,
                private_only: true,
            });
        }
    }

    fn record_member_directional_names(&mut self, group: &Group<'_>) {
        for member in group.members {
            self.record_directional_names(
                group,
                DirectionalNames {
                    serialize: member.attributes.rename_serialize.as_deref(),
                    deserialize: member.attributes.rename_deserialize.as_deref(),
                    authored_directionally: member.attributes.rename_directional,
                    subject: &member.name,
                },
            );
        }
    }

    fn push_removal(
        &mut self,
        cx: &LateContext<'_>,
        group: &Group<'_>,
        direction: SerdeDirection,
        removal: Removal,
    ) {
        let suggestion = standalone_attribute_span(cx, group.span, &removal.expected)
            .map(|span| (span, String::new()));
        self.candidates.push(Candidate {
            definition: group.definition,
            owner: group.owner,
            span: suggestion.as_ref().map_or(group.span, |(span, _)| *span),
            activation: Activation::Direction(direction),
            detail: removal.detail,
            remediation: removal.remediation.to_owned(),
            suggestion,
            private_only: removal.private_only,
        });
    }

    fn record_inherited(
        &mut self,
        cx: &LateContext<'_>,
        group: &Group<'_>,
        active: &[&Member],
        direction: SerdeDirection,
        rule: &str,
    ) {
        for member in active {
            let Some(rename) = Self::explicit_name(member, direction) else {
                continue;
            };
            if rename != member.inherited_name(Some(rule)) {
                continue;
            }
            let symmetric = member.attributes.rename_serialize.as_deref() == Some(rename)
                && member.attributes.rename_deserialize.as_deref() == Some(rename);
            let expected = symmetric.then(|| format!("#[serde(rename=\"{rename}\")]"));
            let suggestion = expected.and_then(|expected| {
                standalone_attribute_span(cx, group.span, &expected)
                    .map(|span| (span, String::new()))
            });
            self.candidates.push(Candidate {
                definition: group.definition,
                owner: group.owner,
                span: suggestion.as_ref().map_or(group.span, |(span, _)| *span),
                activation: Activation::Direction(direction),
                detail: format!(
                    "`{}` already receives `{rename}` from the inherited `{rule}` policy",
                    member.name
                ),
                remediation: "remove the redundant member rename".to_owned(),
                suggestion,
                private_only: false,
            });
        }
    }

    fn record_factoring(
        &mut self,
        group: &Group<'_>,
        active: &[&Member],
        effective: &[String],
        authored: usize,
        direction: SerdeDirection,
    ) {
        let policies = CASES.map(|case| CandidatePolicy {
            name: case,
            names: active
                .iter()
                .map(|member| member.inherited_name(Some(case)))
                .collect(),
            directive_cost: 1,
        });
        let Some(policy) = factor_names(effective, authored, policies) else {
            return;
        };
        let exceptions = policy
            .exceptions
            .iter()
            .map(|index| active[*index].name.as_str())
            .collect::<Vec<_>>();
        let exception_text = if exceptions.is_empty() {
            String::new()
        } else {
            format!(" and keep overrides only for {}", exceptions.join(", "))
        };
        self.candidates.push(Candidate {
            definition: group.definition,
            owner: group.owner,
            span: group.span,
            activation: Activation::Direction(direction),
            detail: format!(
                "{authored} member renames reduce to one `{}` default with {} exception(s)",
                policy.name,
                policy.exceptions.len()
            ),
            remediation: format!(
                "declare `{} = \"{}\"` for {}{exception_text}",
                group.scope,
                policy.name,
                direction.label()
            ),
            suggestion: None,
            private_only: false,
        });
    }

    fn record_identity(
        &mut self,
        cx: &LateContext<'_>,
        group: &Group<'_>,
        active: &[&Member],
        direction: SerdeDirection,
    ) {
        for member in active {
            let Some(rename) = Self::explicit_name(member, direction) else {
                continue;
            };
            if rename == member.rust_name() {
                self.push_removal(
                    cx,
                    group,
                    direction,
                    Removal {
                        expected: format!("#[serde(rename=\"{rename}\")]"),
                        detail: format!(
                            "`{}` is renamed to its unchanged Rust spelling",
                            member.name
                        ),
                        remediation: "remove the behavior-neutral member rename",
                        private_only: true,
                    },
                );
            }
        }
    }

    fn record_aliases(
        &mut self,
        cx: &LateContext<'_>,
        group: &Group<'_>,
        active: &[&Member],
        effective: &[String],
    ) {
        for (index, member) in active.iter().enumerate() {
            let mut seen = HashSet::new();
            for alias in &member.attributes.aliases {
                if alias != &effective[index] && seen.insert(alias) {
                    continue;
                }
                self.push_removal(
                    cx,
                    group,
                    SerdeDirection::Deserialize,
                    Removal {
                        expected: format!("#[serde(alias=\"{alias}\")]"),
                        detail: format!(
                            "alias `{alias}` accepts no additional spelling for `{}`",
                            member.name
                        ),
                        remediation: "remove the redundant Serde alias",
                        private_only: false,
                    },
                );
            }
        }
    }

    fn record_group(&mut self, cx: &LateContext<'_>, group: &Group<'_>) {
        self.record_member_directional_names(group);
        for (direction, inherited) in [SerdeDirection::Serialize, SerdeDirection::Deserialize]
            .into_iter()
            .zip(group.inherited)
        {
            let active = group
                .members
                .iter()
                .filter(|member| match direction {
                    SerdeDirection::Serialize => {
                        !member.attributes.has_flag(SerdeFlag::SkipSerialize)
                    }
                    SerdeDirection::Deserialize => {
                        !member.attributes.has_flag(SerdeFlag::SkipDeserialize)
                    }
                })
                .collect::<Vec<_>>();
            let effective = active
                .iter()
                .map(|member| {
                    Self::explicit_name(member, direction)
                        .map_or_else(|| member.inherited_name(inherited), str::to_owned)
                })
                .collect::<Vec<_>>();
            let authored = active
                .iter()
                .filter(|member| Self::explicit_name(member, direction).is_some())
                .count();
            if let Some(rule) = inherited {
                self.record_inherited(cx, group, &active, direction, rule);
            } else {
                self.record_factoring(group, &active, &effective, authored, direction);
                self.record_identity(cx, group, &active, direction);
            }
            if matches!(direction, SerdeDirection::Deserialize) {
                self.record_aliases(cx, group, &active, &effective);
            }
        }
    }

    fn supports_container_field_policy(variants: &[(Span, SerdeAttributes, Vec<Member>)]) -> bool {
        variants.iter().all(|(_, attributes, _)| {
            attributes.rename_all_serialize.is_none() && attributes.rename_all_deserialize.is_none()
        })
    }

    fn analyze_enum_fields(
        &mut self,
        cx: &LateContext<'_>,
        item: &Item<'_>,
        enumeration: &syn::ItemEnum,
        definition: &EnumDef<'_>,
        container: &SerdeAttributes,
    ) {
        let variant_fields = enumeration
            .variants
            .iter()
            .zip(definition.variants)
            .map(|(variant, hir_variant)| {
                let attributes = SerdeAttributes::from_attributes(&variant.attrs);
                let fields = variant
                    .fields
                    .iter()
                    .zip(hir_variant.data.fields())
                    .filter_map(|(field, _hir)| {
                        Some(Member {
                            name: field.ident.as_ref()?.to_string(),
                            attributes: SerdeAttributes::from_attributes(&field.attrs),
                            is_variant: false,
                        })
                    })
                    .collect::<Vec<_>>();
                (hir_variant.span, attributes, fields)
            })
            .collect::<Vec<_>>();
        if Self::supports_container_field_policy(&variant_fields) {
            let fields = variant_fields
                .into_iter()
                .flat_map(|(_, _, fields)| fields)
                .collect::<Vec<_>>();
            if !fields.is_empty() {
                let group = Group {
                    definition: item.owner_id.def_id,
                    owner: item.hir_id(),
                    span: item.span,
                    scope: "rename_all_fields",
                    inherited: [
                        container.rename_all_fields_serialize.as_deref(),
                        container.rename_all_fields_deserialize.as_deref(),
                    ],
                    members: &fields,
                };
                self.record_directional_names(
                    &group,
                    DirectionalNames {
                        serialize: container.rename_all_fields_serialize.as_deref(),
                        deserialize: container.rename_all_fields_deserialize.as_deref(),
                        authored_directionally: container.rename_all_fields_directional,
                        subject: "the container `rename_all_fields` policy",
                    },
                );
                self.record_group(cx, &group);
            }
            return;
        }
        for (span, variant_attributes, fields) in variant_fields {
            if fields.is_empty() {
                continue;
            }
            let serialize = variant_attributes
                .rename_all_serialize
                .as_deref()
                .or(container.rename_all_fields_serialize.as_deref());
            let deserialize = variant_attributes
                .rename_all_deserialize
                .as_deref()
                .or(container.rename_all_fields_deserialize.as_deref());
            let group = Group {
                definition: item.owner_id.def_id,
                owner: item.hir_id(),
                span,
                scope: "rename_all",
                inherited: [serialize, deserialize],
                members: &fields,
            };
            self.record_directional_names(
                &group,
                DirectionalNames {
                    serialize: variant_attributes.rename_all_serialize.as_deref(),
                    deserialize: variant_attributes.rename_all_deserialize.as_deref(),
                    authored_directionally: variant_attributes.rename_all_directional,
                    subject: "the variant `rename_all` policy",
                },
            );
            self.record_group(cx, &group);
        }
    }

    fn analyze_struct(&mut self, cx: &LateContext<'_>, item: &Item<'_>, source: &str) {
        let ItemKind::Struct(_, _, data) = item.kind else {
            return;
        };
        let Ok(structure) = syn::parse_str::<syn::ItemStruct>(source) else {
            return;
        };
        let container = SerdeAttributes::from_attributes(&structure.attrs);
        let members = structure
            .fields
            .iter()
            .zip(data.fields())
            .filter_map(|(field, _hir)| {
                Some(Member {
                    name: field.ident.as_ref()?.to_string(),
                    attributes: SerdeAttributes::from_attributes(&field.attrs),
                    is_variant: false,
                })
            })
            .collect::<Vec<_>>();
        let group = Group {
            definition: item.owner_id.def_id,
            owner: item.hir_id(),
            span: item.span,
            scope: "rename_all",
            inherited: [
                container.rename_all_serialize.as_deref(),
                container.rename_all_deserialize.as_deref(),
            ],
            members: &members,
        };
        self.record_directional_names(
            &group,
            DirectionalNames {
                serialize: container.rename_all_serialize.as_deref(),
                deserialize: container.rename_all_deserialize.as_deref(),
                authored_directionally: container.rename_all_directional,
                subject: "the container `rename_all` policy",
            },
        );
        self.record_group(cx, &group);
    }

    fn analyze_enum(&mut self, cx: &LateContext<'_>, item: &Item<'_>, source: &str) {
        let ItemKind::Enum(_, _, definition) = item.kind else {
            return;
        };
        let Ok(enumeration) = syn::parse_str::<syn::ItemEnum>(source) else {
            return;
        };
        let container = SerdeAttributes::from_attributes(&enumeration.attrs);
        if !container.has_flag(SerdeFlag::Untagged) {
            let variants = enumeration
                .variants
                .iter()
                .zip(definition.variants)
                .map(|(variant, _hir)| Member {
                    name: variant.ident.to_string(),
                    attributes: SerdeAttributes::from_attributes(&variant.attrs),
                    is_variant: true,
                })
                .collect::<Vec<_>>();
            let group = Group {
                definition: item.owner_id.def_id,
                owner: item.hir_id(),
                span: item.span,
                scope: "rename_all",
                inherited: [
                    container.rename_all_serialize.as_deref(),
                    container.rename_all_deserialize.as_deref(),
                ],
                members: &variants,
            };
            self.record_directional_names(
                &group,
                DirectionalNames {
                    serialize: container.rename_all_serialize.as_deref(),
                    deserialize: container.rename_all_deserialize.as_deref(),
                    authored_directionally: container.rename_all_directional,
                    subject: "the container `rename_all` policy",
                },
            );
            self.record_group(cx, &group);
        }
        self.analyze_enum_fields(cx, item, &enumeration, &definition, &container);
    }
}

impl LateLintPass<'_> for SerdeNoncanonicalRenamePolicies {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
        if item.span.from_expansion() {
            return;
        }
        let Some(source) = AuthoredItemSource::for_item(cx, item) else {
            return;
        };
        match item.kind {
            ItemKind::Struct(..) => self.analyze_struct(cx, item, &source),
            ItemKind::Enum(..) => self.analyze_enum(cx, item, &source),
            _ => {}
        }
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for candidate in self.candidates.drain(..) {
            let has_derive = |direction| {
                let derive = match direction {
                    SerdeDirection::Serialize => "Serialize",
                    SerdeDirection::Deserialize => "Deserialize",
                };
                self.catalog
                    .derived_type(candidate.definition, derive)
                    .is_some()
            };
            let serialize = has_derive(SerdeDirection::Serialize);
            let deserialize = has_derive(SerdeDirection::Deserialize);
            let active = match candidate.activation {
                Activation::Direction(SerdeDirection::Serialize) => serialize,
                Activation::Direction(SerdeDirection::Deserialize) => deserialize,
                Activation::Both => serialize && deserialize,
                Activation::Only(SerdeDirection::Serialize) => serialize && !deserialize,
                Activation::Only(SerdeDirection::Deserialize) => deserialize && !serialize,
            };
            let exported = cx
                .tcx
                .effective_visibilities(())
                .is_exported(candidate.definition);
            if active && !(candidate.private_only && exported) {
                candidate.emit(cx);
            }
        }
    }
}
