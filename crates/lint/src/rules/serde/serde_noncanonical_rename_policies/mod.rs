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
use crate::utils::name_policy::{
    CandidatePolicy, FactoredPolicy, factor_names, standalone_attribute_span,
};
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
    Any,
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

    fn participates(member: &Member, direction: SerdeDirection) -> bool {
        match direction {
            SerdeDirection::Serialize => !member.attributes.has_flag(SerdeFlag::SkipSerialize),
            SerdeDirection::Deserialize => !member.attributes.has_flag(SerdeFlag::SkipDeserialize),
        }
    }

    fn symmetric_removal_preserves_both_directions(
        group: &Group<'_>,
        member: &Member,
        rename: &str,
    ) -> bool {
        [SerdeDirection::Serialize, SerdeDirection::Deserialize]
            .into_iter()
            .zip(group.inherited)
            .all(|(direction, inherited)| {
                !Self::participates(member, direction) || member.inherited_name(inherited) == rename
            })
    }

    fn is_primary_participating_direction(member: &Member, direction: SerdeDirection) -> bool {
        match direction {
            SerdeDirection::Serialize => Self::participates(member, SerdeDirection::Serialize),
            SerdeDirection::Deserialize => {
                !Self::participates(member, SerdeDirection::Serialize)
                    && Self::participates(member, SerdeDirection::Deserialize)
            }
        }
    }

    fn record_directional_names(&mut self, group: &Group<'_>, names: DirectionalNames<'_>) {
        if !names.authored_directionally {
            return;
        }
        if names.serialize.is_some() && names.serialize == names.deserialize {
            for (activation, detail, remediation, private_only) in [
                (
                    Activation::Both,
                    format!(
                        "{} declares identical serialization and deserialization names",
                        names.subject
                    ),
                    "use the nondirectional Serde shorthand for the shared name".to_owned(),
                    false,
                ),
                (
                    Activation::Only(SerdeDirection::Serialize),
                    format!(
                        "{} configures deserialization, but this private type derives only serialization",
                        names.subject
                    ),
                    "remove the inactive deserialization naming branch".to_owned(),
                    true,
                ),
                (
                    Activation::Only(SerdeDirection::Deserialize),
                    format!(
                        "{} configures serialization, but this private type derives only deserialization",
                        names.subject
                    ),
                    "remove the inactive serialization naming branch".to_owned(),
                    true,
                ),
            ] {
                self.candidates.push(Candidate {
                    definition: group.definition,
                    owner: group.owner,
                    span: group.span,
                    activation,
                    detail,
                    remediation,
                    suggestion: None,
                    private_only,
                });
            }
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

    fn record_skipped_member_directions(
        &mut self,
        group: &Group<'_>,
        member: &Member,
        serialize: Option<&str>,
        deserialize: Option<&str>,
        exported: bool,
    ) -> bool {
        let participates = [
            Self::participates(member, SerdeDirection::Serialize),
            Self::participates(member, SerdeDirection::Deserialize),
        ];
        let authored = [serialize, deserialize];
        if authored.iter().all(Option::is_none) {
            return false;
        }
        match participates {
            [true, true] => false,
            [false, false] => {
                self.candidates.push(Candidate {
                    definition: group.definition,
                    owner: group.owner,
                    span: group.span,
                    activation: Activation::Any,
                    detail: format!(
                        "{} is skipped during both serialization and deserialization, so its member rename is inactive",
                        member.name
                    ),
                    remediation: "remove the inactive member rename".to_owned(),
                    suggestion: None,
                    private_only: false,
                });
                true
            }
            participation => {
                let (active, inactive, inactive_name, inactive_label) = if participation[0] {
                    (
                        SerdeDirection::Serialize,
                        SerdeDirection::Deserialize,
                        deserialize,
                        "deserialization",
                    )
                } else {
                    (
                        SerdeDirection::Deserialize,
                        SerdeDirection::Serialize,
                        serialize,
                        "serialization",
                    )
                };
                let active_name = match active {
                    SerdeDirection::Serialize => serialize,
                    SerdeDirection::Deserialize => deserialize,
                };
                let active_inherited = match active {
                    SerdeDirection::Serialize => group.inherited[0],
                    SerdeDirection::Deserialize => group.inherited[1],
                };
                let active_name_is_redundant = active_name.is_none_or(|name| {
                    (active_inherited.is_some() && member.inherited_name(active_inherited) == name)
                        || (active_inherited.is_none() && !exported && member.rust_name() == name)
                });
                if member.attributes.rename_directional
                    && inactive_name.is_some()
                    && !active_name_is_redundant
                {
                    self.candidates.push(Candidate {
                        definition: group.definition,
                        owner: group.owner,
                        span: group.span,
                        activation: Activation::Direction(active),
                        detail: format!(
                            "{} is skipped during {inactive_label}, so its {inactive_label} name is inactive",
                            member.name
                        ),
                        remediation: format!(
                            "remove the inactive {inactive_label} naming branch"
                        ),
                        suggestion: None,
                        private_only: false,
                    });
                }
                self.candidates.push(Candidate {
                    definition: group.definition,
                    owner: group.owner,
                    span: group.span,
                    activation: Activation::Only(inactive),
                    detail: format!(
                        "{} has no active naming direction because {} is not derived and it is skipped during {inactive_label}",
                        member.name,
                        active.label()
                    ),
                    remediation: "remove the inactive member rename".to_owned(),
                    suggestion: None,
                    private_only: false,
                });
                member.attributes.rename_directional || active_name_is_redundant
            }
        }
    }

    fn record_member_directional_names(&mut self, cx: &LateContext<'_>, group: &Group<'_>) {
        let exported = cx
            .tcx
            .effective_visibilities(())
            .is_exported(group.definition);
        for member in group.members {
            let serialize = member.attributes.rename_serialize.as_deref();
            let deserialize = member.attributes.rename_deserialize.as_deref();
            let shared = if serialize.is_some() && serialize == deserialize {
                serialize
            } else {
                None
            };
            let removal_takes_precedence = shared.is_some_and(|shared| {
                Self::symmetric_removal_preserves_both_directions(group, member, shared)
                    && if Self::participates(member, SerdeDirection::Serialize) {
                        group.inherited[0].is_some() || !exported
                    } else if Self::participates(member, SerdeDirection::Deserialize) {
                        group.inherited[1].is_some() || !exported
                    } else {
                        false
                    }
            });
            if removal_takes_precedence {
                continue;
            }
            if self.record_skipped_member_directions(
                group,
                member,
                serialize,
                deserialize,
                exported,
            ) {
                continue;
            }
            self.record_directional_names(
                group,
                DirectionalNames {
                    serialize,
                    deserialize,
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

    fn push_symmetric_removal(
        &mut self,
        cx: &LateContext<'_>,
        group: &Group<'_>,
        removal: Removal,
    ) {
        let suggestion = standalone_attribute_span(cx, group.span, &removal.expected)
            .map(|span| (span, String::new()));
        for activation in [
            Activation::Both,
            Activation::Only(SerdeDirection::Serialize),
            Activation::Only(SerdeDirection::Deserialize),
        ] {
            self.candidates.push(Candidate {
                definition: group.definition,
                owner: group.owner,
                span: suggestion.as_ref().map_or(group.span, |(span, _)| *span),
                activation,
                detail: removal.detail.clone(),
                remediation: removal.remediation.to_owned(),
                suggestion: suggestion.clone(),
                private_only: removal.private_only,
            });
        }
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
            if symmetric
                && !Self::symmetric_removal_preserves_both_directions(group, member, rename)
            {
                continue;
            }
            let detail = format!(
                "`{}` already receives `{rename}` from the inherited `{rule}` policy",
                member.name
            );
            if symmetric {
                if !Self::is_primary_participating_direction(member, direction) {
                    continue;
                }
                self.push_symmetric_removal(
                    cx,
                    group,
                    Removal {
                        expected: format!("#[serde(rename=\"{rename}\")]"),
                        detail,
                        remediation: "remove the redundant member rename",
                        private_only: false,
                    },
                );
                continue;
            }
            self.candidates.push(Candidate {
                definition: group.definition,
                owner: group.owner,
                span: group.span,
                activation: Activation::Direction(direction),
                detail,
                remediation: "remove the redundant member rename".to_owned(),
                suggestion: None,
                private_only: false,
            });
        }
    }

    fn factoring_policy(
        active: &[&Member],
        effective: &[String],
        authored: usize,
    ) -> Option<FactoredPolicy> {
        let policies = CASES.map(|case| CandidatePolicy {
            name: case,
            names: active
                .iter()
                .map(|member| member.inherited_name(Some(case)))
                .collect(),
            directive_cost: 1,
        });
        factor_names(effective, authored, policies)
    }

    fn push_factoring(
        &mut self,
        group: &Group<'_>,
        active: &[&Member],
        authored: usize,
        policy: &FactoredPolicy,
        activation: Activation,
        direction: Option<SerdeDirection>,
    ) {
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
        let remediation = if let Some(direction) = direction {
            format!(
                "declare `{} = \"{}\"` for {}{exception_text}",
                group.scope,
                policy.name,
                direction.label()
            )
        } else {
            format!(
                "declare nondirectional `{} = \"{}\"`{exception_text}",
                group.scope, policy.name
            )
        };
        self.candidates.push(Candidate {
            definition: group.definition,
            owner: group.owner,
            span: group.span,
            activation,
            detail: format!(
                "{authored} member renames reduce to one `{}` default with {} exception(s)",
                policy.name,
                policy.exceptions.len()
            ),
            remediation,
            suggestion: None,
            private_only: false,
        });
    }

    fn record_factoring(
        &mut self,
        group: &Group<'_>,
        active: &[&Member],
        effective: &[String],
        authored: usize,
        direction: SerdeDirection,
    ) {
        let Some(policy) = Self::factoring_policy(active, effective, authored) else {
            return;
        };
        self.push_factoring(
            group,
            active,
            authored,
            &policy,
            Activation::Direction(direction),
            Some(direction),
        );
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
                let symmetric = member.attributes.rename_serialize.as_deref() == Some(rename)
                    && member.attributes.rename_deserialize.as_deref() == Some(rename);
                if symmetric
                    && !Self::symmetric_removal_preserves_both_directions(group, member, rename)
                {
                    continue;
                }
                let removal = Removal {
                    expected: format!("#[serde(rename=\"{rename}\")]"),
                    detail: format!(
                        "`{}` is renamed to its unchanged Rust spelling",
                        member.name
                    ),
                    remediation: "remove the behavior-neutral member rename",
                    private_only: true,
                };
                if symmetric {
                    if Self::is_primary_participating_direction(member, direction) {
                        self.push_symmetric_removal(cx, group, removal);
                    }
                    continue;
                }
                self.push_removal(cx, group, direction, removal);
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
        self.record_member_directional_names(cx, group);
        let mut combined_factoring = false;
        if group.inherited == [None, None] {
            let serialize = group
                .members
                .iter()
                .filter(|member| Self::participates(member, SerdeDirection::Serialize))
                .collect::<Vec<_>>();
            let deserialize = group
                .members
                .iter()
                .filter(|member| Self::participates(member, SerdeDirection::Deserialize))
                .collect::<Vec<_>>();
            let same_members = serialize
                .iter()
                .map(|member| member.name.as_str())
                .eq(deserialize.iter().map(|member| member.name.as_str()));
            if same_members {
                let serialize_effective = serialize
                    .iter()
                    .map(|member| {
                        Self::explicit_name(member, SerdeDirection::Serialize)
                            .map_or_else(|| member.inherited_name(None), str::to_owned)
                    })
                    .collect::<Vec<_>>();
                let deserialize_effective = deserialize
                    .iter()
                    .map(|member| {
                        Self::explicit_name(member, SerdeDirection::Deserialize)
                            .map_or_else(|| member.inherited_name(None), str::to_owned)
                    })
                    .collect::<Vec<_>>();
                let serialize_authored = serialize
                    .iter()
                    .filter(|member| {
                        Self::explicit_name(member, SerdeDirection::Serialize).is_some()
                    })
                    .count();
                let deserialize_authored = deserialize
                    .iter()
                    .filter(|member| {
                        Self::explicit_name(member, SerdeDirection::Deserialize).is_some()
                    })
                    .count();
                if serialize_effective == deserialize_effective
                    && serialize_authored == deserialize_authored
                    && let Some(policy) =
                        Self::factoring_policy(&serialize, &serialize_effective, serialize_authored)
                {
                    self.push_factoring(
                        group,
                        &serialize,
                        serialize_authored,
                        &policy,
                        Activation::Both,
                        None,
                    );
                    self.push_factoring(
                        group,
                        &serialize,
                        serialize_authored,
                        &policy,
                        Activation::Only(SerdeDirection::Serialize),
                        Some(SerdeDirection::Serialize),
                    );
                    self.push_factoring(
                        group,
                        &serialize,
                        serialize_authored,
                        &policy,
                        Activation::Only(SerdeDirection::Deserialize),
                        Some(SerdeDirection::Deserialize),
                    );
                    combined_factoring = true;
                }
            }
        }
        for (direction, inherited) in [SerdeDirection::Serialize, SerdeDirection::Deserialize]
            .into_iter()
            .zip(group.inherited)
        {
            let active = group
                .members
                .iter()
                .filter(|member| Self::participates(member, direction))
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
                if !combined_factoring {
                    self.record_factoring(group, &active, &effective, authored, direction);
                }
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
                Activation::Any => serialize || deserialize,
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
