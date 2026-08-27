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
    AuthoredAttributeSpanExt as _, NamingPolicyCandidate, NamingPolicyFactor,
};
use crate::utils::source_provenance::AuthoredItemSource;

// -----------------------------------------------------------------------------
// Cases: Serde naming policies
// -----------------------------------------------------------------------------

/// Serde naming policies considered for container factoring.
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

// -----------------------------------------------------------------------------
// Suggestion: Machine-applicable source edit
// -----------------------------------------------------------------------------

/// Exact source replacement for a redundant attribute.
#[derive(Clone)]
struct Suggestion {
    /// Authored source range to replace.
    span: Span,
    /// Replacement source text.
    replacement: String,
}

// -----------------------------------------------------------------------------
// Candidate: Deferred rename-policy violation
// -----------------------------------------------------------------------------

/// Potential violation retained until the generated Serde contract is known.
struct Candidate {
    /// Local type definition used for derive contract lookup.
    definition: LocalDefId,
    /// HIR owner receiving the diagnostic.
    owner: HirId,
    /// Primary diagnostic span.
    span: Span,
    /// Generated derive state required to activate this candidate.
    activation: Activation,
    /// Explanation of the redundant policy.
    detail: String,
    /// Canonical policy requested from the author.
    remediation: String,
    /// Safe edit for a uniquely located redundant attribute.
    suggestion: Option<Suggestion>,
    /// Whether the candidate is valid only for a non-exported type.
    is_private_only: bool,
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
                if let Some(suggestion) = self.suggestion {
                    diag.span_suggestion(
                        suggestion.span,
                        remediation,
                        suggestion.replacement,
                        Applicability::MachineApplicable,
                    );
                } else {
                    diag.help(remediation);
                }
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// Activation: Required derive direction
// -----------------------------------------------------------------------------

/// Generated Serde derive state required for a candidate to be actionable.
#[derive(Clone, Copy)]
enum Activation {
    /// Either or both Serde directions may be active.
    Any,
    /// The selected direction must be active.
    Direction(
        /// Direction whose generated implementation must exist.
        SerdeDirection,
    ),
    /// Both serialization and deserialization must be active.
    Both,
    /// Exactly the selected direction must be active.
    Only(
        /// Sole generated direction required by this candidate.
        SerdeDirection,
    ),
}

// -----------------------------------------------------------------------------
// Exposure: Downstream visibility contract
// -----------------------------------------------------------------------------

/// Whether downstream crates can observe a type's serialized wire names.
#[derive(Clone, Copy)]
enum Exposure {
    /// Effective visibility reaches outside the current crate.
    Exported,
    /// Effective visibility is confined to the current crate.
    Private,
}

impl Exposure {
    /// Classifies a local definition from rustc's effective visibility analysis.
    fn for_definition(cx: &LateContext<'_>, definition: LocalDefId) -> Self {
        if cx.tcx.effective_visibilities(()).is_exported(definition) {
            Self::Exported
        } else {
            Self::Private
        }
    }

    /// Returns whether downstream crates can depend on this type's wire names.
    const fn is_exported(self) -> bool {
        matches!(self, Self::Exported)
    }
}

// -----------------------------------------------------------------------------
// Member: Authored field or variant
// -----------------------------------------------------------------------------

/// One field or variant participating in a naming-policy group.
struct Member {
    /// Authored Rust identifier.
    name: String,
    /// Parsed Serde attributes on this member.
    attributes: SerdeAttributes,
    /// Whether the member is an enum variant rather than a field.
    is_variant: bool,
}

impl Member {
    /// Returns the identifier without a raw-identifier prefix.
    fn rust_name(&self) -> &str {
        self.name.strip_prefix("r#").unwrap_or(&self.name)
    }

    /// Applies the inherited Serde case policy appropriate to this member kind.
    fn inherited_name(&self, rule: Option<&str>) -> String {
        if self.is_variant {
            SerdeCase::apply_to_variant(self.rust_name(), rule)
        } else {
            SerdeCase::apply_to_field(self.rust_name(), rule)
        }
    }
}

// -----------------------------------------------------------------------------
// VariantFields: One enum variant's field-policy scope
// -----------------------------------------------------------------------------

/// Parsed attributes and named fields owned by one enum variant.
struct VariantFields {
    /// HIR span used for variant-local diagnostics.
    span: Span,
    /// Serde policy authored on the variant.
    attributes: SerdeAttributes,
    /// Named fields governed by the variant policy.
    fields: Vec<Member>,
}

// -----------------------------------------------------------------------------
// Group: Shared naming-policy scope
// -----------------------------------------------------------------------------

/// Container and members governed by one pair of inherited direction policies.
struct Group<'a> {
    /// Local type definition used for derive contract lookup.
    definition: LocalDefId,
    /// HIR owner receiving diagnostics for this group.
    owner: HirId,
    /// Authored container span.
    span: Span,
    /// Human-readable scope name used in diagnostics.
    scope: &'a str,
    /// Serialize and deserialize inherited case rules.
    inherited: [Option<&'a str>; 2],
    /// Members governed by the inherited policies.
    members: &'a [Member],
}

// -----------------------------------------------------------------------------
// DirectionalNames: Authored direction pair
// -----------------------------------------------------------------------------

/// Serialize and deserialize names authored for one semantic subject.
#[derive(Clone, Copy)]
struct DirectionalNames<'a> {
    /// Serialization name when explicitly authored.
    serialize: Option<&'a str>,
    /// Deserialization name when explicitly authored.
    deserialize: Option<&'a str>,
    /// Whether separate directional syntax was used.
    is_authored_directionally: bool,
    /// Human-readable subject used in diagnostics.
    subject: &'a str,
}

// -----------------------------------------------------------------------------
// ActiveDirectionView: One participating direction
// -----------------------------------------------------------------------------

/// Direction-local view of authored names and inherited policy.
struct ActiveDirectionView<'names, 'policy> {
    /// Participating Serde direction.
    active: SerdeDirection,
    /// Complementary skipped direction.
    inactive: SerdeDirection,
    /// Name authored for the participating direction.
    active_name: Option<&'names str>,
    /// Name authored for the skipped direction.
    inactive_name: Option<&'names str>,
    /// Human-readable skipped direction.
    inactive_label: &'static str,
    /// Container policy inherited by the participating direction.
    inherited: Option<&'policy str>,
}

impl<'names, 'policy> ActiveDirectionView<'names, 'policy> {
    /// Resolves paired names and policies for one participating direction.
    fn resolve(
        active: SerdeDirection,
        serialize: Option<&'names str>,
        deserialize: Option<&'names str>,
        inherited: [Option<&'policy str>; 2],
    ) -> Self {
        match active {
            SerdeDirection::Serialize => Self {
                active,
                inactive: SerdeDirection::Deserialize,
                active_name: serialize,
                inactive_name: deserialize,
                inactive_label: "deserialization",
                inherited: inherited[0],
            },
            SerdeDirection::Deserialize => Self {
                active,
                inactive: SerdeDirection::Serialize,
                active_name: deserialize,
                inactive_name: serialize,
                inactive_label: "serialization",
                inherited: inherited[1],
            },
        }
    }
}

// -----------------------------------------------------------------------------
// MemberParticipation: Directional participation shape
// -----------------------------------------------------------------------------

/// Which Serde directions include one member.
enum MemberParticipation {
    /// Both directions include the member.
    BothActive,
    /// Both directions skip the member.
    BothSkipped,
    /// Exactly one direction includes the member.
    Active {
        /// Sole participating direction.
        direction: SerdeDirection,
    },
}

// -----------------------------------------------------------------------------
// Removal: Redundant attribute edit
// -----------------------------------------------------------------------------

/// Description of one redundant authored attribute eligible for removal.
struct Removal {
    /// Whitespace-normalized attribute spelling to locate.
    expected: String,
    /// Explanation of why the attribute is redundant.
    detail: String,
    /// Static remediation text.
    remediation: &'static str,
    /// Whether removal is safe only for a non-exported type.
    is_private_only: bool,
}

// -----------------------------------------------------------------------------
// SerdeNoncanonicalRenamePolicies: Lint pass
// -----------------------------------------------------------------------------

/// Collects authored naming policies and validates them against generated derives.
#[derive(Default)]
struct SerdeNoncanonicalRenamePolicies {
    /// Generated Serde contract catalog.
    catalog: SerdeContractCatalog,
    /// Potential violations awaiting derive-state activation.
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
    /// Returns a member's explicitly authored name for one Serde direction.
    fn explicit_name(member: &Member, direction: SerdeDirection) -> Option<&str> {
        match direction {
            SerdeDirection::Serialize => member.attributes.rename_serialize.as_deref(),
            SerdeDirection::Deserialize => member.attributes.rename_deserialize.as_deref(),
        }
    }

    /// Returns whether a member participates in one Serde direction.
    fn is_participating(member: &Member, direction: SerdeDirection) -> bool {
        match direction {
            SerdeDirection::Serialize => !member.attributes.has_flag(SerdeFlag::SkipSerialize),
            SerdeDirection::Deserialize => !member.attributes.has_flag(SerdeFlag::SkipDeserialize),
        }
    }

    /// Returns whether removing a shared rename preserves both generated directions.
    fn is_symmetric_removal_preserving_both_directions(
        group: &Group<'_>,
        member: &Member,
        rename: &str,
    ) -> bool {
        [SerdeDirection::Serialize, SerdeDirection::Deserialize]
            .into_iter()
            .zip(group.inherited)
            .all(|(direction, inherited)| {
                !Self::is_participating(member, direction)
                    || member.inherited_name(inherited) == rename
            })
    }

    /// Returns whether a direction is the member's first active generated contract.
    fn is_primary_participating_direction(member: &Member, direction: SerdeDirection) -> bool {
        match direction {
            SerdeDirection::Serialize => Self::is_participating(member, SerdeDirection::Serialize),
            SerdeDirection::Deserialize => {
                !Self::is_participating(member, SerdeDirection::Serialize)
                    && Self::is_participating(member, SerdeDirection::Deserialize)
            }
        }
    }

    /// Selects a cheaper unambiguous container policy for effective member names.
    fn factoring_policy(
        active: &[&Member],
        effective: &[String],
        authored: usize,
    ) -> Option<NamingPolicyFactor> {
        let policies = CASES.map(|case| NamingPolicyCandidate {
            name: case,
            names: active
                .iter()
                .map(|member| member.inherited_name(Some(case)))
                .collect(),
            directive_cost: 1,
        });
        NamingPolicyFactor::factor(effective, authored, policies)
    }

    /// Returns whether enum variants permit one enum-wide field naming policy.
    fn has_container_field_policy_support(variants: &[VariantFields]) -> bool {
        variants.iter().all(|variant| {
            variant.attributes.rename_all_serialize.is_none()
                && variant.attributes.rename_all_deserialize.is_none()
        })
    }

    /// Records redundant or inactive branches in directional naming syntax.
    fn record_directional_names(&mut self, group: &Group<'_>, names: DirectionalNames<'_>) {
        // Shared syntax is already canonical when no directional form was authored.
        if !names.is_authored_directionally {
            return;
        }

        // Identical branches are fully represented by shared and inactive-branch candidates.
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
                    is_private_only: private_only,
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
                is_private_only: true,
            });
        }
    }

    /// Records inactive naming state when exactly one Serde direction participates.
    fn has_recorded_active_branch(
        &mut self,
        group: &Group<'_>,
        member: &Member,
        names: [Option<&str>; 2],
        exposure: Exposure,
        active: SerdeDirection,
    ) -> bool {
        let view = ActiveDirectionView::resolve(active, names[0], names[1], group.inherited);

        // A name authored only for the skipped direction has no generated effect.
        if view.active_name.is_none() {
            self.candidates.push(Candidate {
                definition: group.definition,
                owner: group.owner,
                span: group.span,
                activation: Activation::Any,
                detail: format!(
                    "{} defines a name only for skipped {}",
                    member.name, view.inactive_label
                ),
                remediation: "remove the inactive member rename".to_owned(),
                suggestion: None,
                is_private_only: false,
            });
            return true;
        }
        let is_redundant = view.active_name.is_some_and(|name| {
            (view.inherited.is_some() && member.inherited_name(view.inherited) == name)
                || (view.inherited.is_none()
                    && !exposure.is_exported()
                    && member.rust_name() == name)
        });

        // Preserve a useful active branch while removing only the skipped branch.
        if member.attributes.is_rename_directional && view.inactive_name.is_some() && !is_redundant
        {
            self.candidates.push(Candidate {
                definition: group.definition,
                owner: group.owner,
                span: group.span,
                activation: Activation::Direction(view.active),
                detail: format!(
                    "{} is skipped during {}, so its {} name is inactive",
                    member.name, view.inactive_label, view.inactive_label
                ),
                remediation: format!("remove the inactive {} naming branch", view.inactive_label),
                suggestion: None,
                is_private_only: false,
            });
        }
        self.candidates.push(Candidate {
            definition: group.definition,
            owner: group.owner,
            span: group.span,
            activation: Activation::Only(view.inactive),
            detail: format!(
                "{} has no active naming direction because {} is not derived and it is skipped during {}",
                member.name,
                view.active.label(),
                view.inactive_label
            ),
            remediation: "remove the inactive member rename".to_owned(),
            suggestion: None,
            is_private_only: false,
        });
        member.attributes.is_rename_directional || is_redundant
    }

    /// Records an authored rename on a member skipped in both directions.
    fn record_fully_skipped_member(&mut self, group: &Group<'_>, member: &Member) {
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
            is_private_only: false,
        });
    }

    /// Records inactive names for skipped directions and reports whether analysis is complete.
    fn has_recorded_skipped_member_directions(
        &mut self,
        group: &Group<'_>,
        member: &Member,
        serialize: Option<&str>,
        deserialize: Option<&str>,
        exposure: Exposure,
    ) -> bool {
        let participates = [
            Self::is_participating(member, SerdeDirection::Serialize),
            Self::is_participating(member, SerdeDirection::Deserialize),
        ];
        let authored = [serialize, deserialize];

        // Members without authored names need no skipped-direction diagnosis.
        if authored.iter().all(Option::is_none) {
            return false;
        }

        let participation = match participates {
            [true, true] => MemberParticipation::BothActive,
            [false, false] => MemberParticipation::BothSkipped,
            [true, false] => MemberParticipation::Active {
                direction: SerdeDirection::Serialize,
            },
            [false, true] => MemberParticipation::Active {
                direction: SerdeDirection::Deserialize,
            },
        };

        // Each participation shape has one complete diagnostic outcome.
        match participation {
            MemberParticipation::BothActive => false,
            MemberParticipation::BothSkipped => {
                self.record_fully_skipped_member(group, member);
                true
            }
            MemberParticipation::Active { direction } => {
                self.has_recorded_active_branch(group, member, authored, exposure, direction)
            }
        }
    }

    /// Records directional member names that can use shared or active-only syntax.
    fn record_member_directional_names(&mut self, cx: &LateContext<'_>, group: &Group<'_>) {
        let exposure = Exposure::for_definition(cx, group.definition);
        for member in group.members {
            let serialize = member.attributes.rename_serialize.as_deref();
            let deserialize = member.attributes.rename_deserialize.as_deref();
            let shared = if serialize.is_some() && serialize == deserialize {
                serialize
            } else {
                None
            };
            let removal_takes_precedence = shared.is_some_and(|shared| {
                Self::is_symmetric_removal_preserving_both_directions(group, member, shared)
                    && if Self::is_participating(member, SerdeDirection::Serialize) {
                        group.inherited[0].is_some() || !exposure.is_exported()
                    } else if Self::is_participating(member, SerdeDirection::Deserialize) {
                        group.inherited[1].is_some() || !exposure.is_exported()
                    } else {
                        false
                    }
            });
            if removal_takes_precedence {
                continue;
            }
            if self.has_recorded_skipped_member_directions(
                group,
                member,
                serialize,
                deserialize,
                exposure,
            ) {
                continue;
            }
            self.record_directional_names(
                group,
                DirectionalNames {
                    serialize,
                    deserialize,
                    is_authored_directionally: member.attributes.is_rename_directional,
                    subject: &member.name,
                },
            );
        }
    }

    /// Adds a direction-specific attribute removal candidate.
    fn push_removal(
        &mut self,
        cx: &LateContext<'_>,
        group: &Group<'_>,
        direction: SerdeDirection,
        removal: Removal,
    ) {
        let suggestion = group
            .span
            .standalone_attribute(cx, &removal.expected)
            .map(|span| Suggestion {
                span,
                replacement: String::new(),
            });
        self.candidates.push(Candidate {
            definition: group.definition,
            owner: group.owner,
            span: suggestion.as_ref().map_or(group.span, |edit| edit.span),
            activation: Activation::Direction(direction),
            detail: removal.detail,
            remediation: removal.remediation.to_owned(),
            suggestion,
            is_private_only: removal.is_private_only,
        });
    }

    /// Adds equivalent removals for every compatible derive state.
    fn push_symmetric_removal(
        &mut self,
        cx: &LateContext<'_>,
        group: &Group<'_>,
        removal: Removal,
    ) {
        let suggestion = group
            .span
            .standalone_attribute(cx, &removal.expected)
            .map(|span| Suggestion {
                span,
                replacement: String::new(),
            });
        for activation in [
            Activation::Both,
            Activation::Only(SerdeDirection::Serialize),
            Activation::Only(SerdeDirection::Deserialize),
        ] {
            self.candidates.push(Candidate {
                definition: group.definition,
                owner: group.owner,
                span: suggestion.as_ref().map_or(group.span, |edit| edit.span),
                activation,
                detail: removal.detail.clone(),
                remediation: removal.remediation.to_owned(),
                suggestion: suggestion.clone(),
                is_private_only: removal.is_private_only,
            });
        }
    }

    /// Records member names already produced by an inherited policy.
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
                && !Self::is_symmetric_removal_preserving_both_directions(group, member, rename)
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
                        is_private_only: false,
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
                is_private_only: false,
            });
        }
    }

    /// Adds a candidate replacing member renames with one container policy.
    fn push_factoring(
        &mut self,
        group: &Group<'_>,
        active: &[&Member],
        authored: usize,
        policy: &NamingPolicyFactor,
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
            is_private_only: false,
        });
    }

    /// Finds and records an unambiguous cheaper container policy.
    fn record_factoring(
        &mut self,
        group: &Group<'_>,
        active: &[&Member],
        effective: &[String],
        authored: usize,
        direction: SerdeDirection,
    ) {
        // Groups without a cheaper unambiguous policy remain explicitly authored.
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

    /// Records names that redundantly repeat authored Rust identifiers.
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
            if rename != member.rust_name() {
                continue;
            }
            let symmetric = member.attributes.rename_serialize.as_deref() == Some(rename)
                && member.attributes.rename_deserialize.as_deref() == Some(rename);
            if symmetric
                && !Self::is_symmetric_removal_preserving_both_directions(group, member, rename)
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
                is_private_only: true,
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

    /// Records aliases or direction branches without generated behavior.
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
                        is_private_only: false,
                    },
                );
            }
        }
    }

    /// Runs direction-aware rename analysis for one member group.
    fn record_group(&mut self, cx: &LateContext<'_>, group: &Group<'_>) {
        self.record_member_directional_names(cx, group);
        let mut combined_factoring = false;
        if group.inherited == [None, None] {
            let serialize = group
                .members
                .iter()
                .filter(|member| Self::is_participating(member, SerdeDirection::Serialize))
                .collect::<Vec<_>>();
            let deserialize = group
                .members
                .iter()
                .filter(|member| Self::is_participating(member, SerdeDirection::Deserialize))
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
                .filter(|member| Self::is_participating(member, direction))
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
            if !matches!(direction, SerdeDirection::Deserialize) {
                continue;
            }
            self.record_aliases(cx, group, &active, &effective);
        }
    }

    /// Analyzes field naming policies shared across enum variants.
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
                VariantFields {
                    span: hir_variant.span,
                    attributes,
                    fields,
                }
            })
            .collect::<Vec<_>>();

        // Compatible variants share one container field-policy group.
        if Self::has_container_field_policy_support(&variant_fields) {
            let fields = variant_fields
                .into_iter()
                .flat_map(|variant| variant.fields)
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
                        is_authored_directionally: container.is_rename_all_fields_directional,
                        subject: "the container `rename_all_fields` policy",
                    },
                );
                self.record_group(cx, &group);
            }
            return;
        }
        for variant in variant_fields {
            let VariantFields {
                span,
                attributes: variant_attributes,
                fields,
            } = variant;
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
                    is_authored_directionally: variant_attributes.is_rename_all_directional,
                    subject: "the variant `rename_all` policy",
                },
            );
            self.record_group(cx, &group);
        }
    }

    /// Records the naming group constructed from one authored struct.
    fn record_struct_group(
        &mut self,
        cx: &LateContext<'_>,
        item: &Item<'_>,
        container: &SerdeAttributes,
        members: &[Member],
    ) {
        let group = Group {
            definition: item.owner_id.def_id,
            owner: item.hir_id(),
            span: item.span,
            scope: "rename_all",
            inherited: [
                container.rename_all_serialize.as_deref(),
                container.rename_all_deserialize.as_deref(),
            ],
            members,
        };
        self.record_directional_names(
            &group,
            DirectionalNames {
                serialize: container.rename_all_serialize.as_deref(),
                deserialize: container.rename_all_deserialize.as_deref(),
                is_authored_directionally: container.is_rename_all_directional,
                subject: "the container `rename_all` policy",
            },
        );
        self.record_group(cx, &group);
    }

    /// Parses and analyzes one authored struct.
    fn analyze_struct(&mut self, cx: &LateContext<'_>, item: &Item<'_>, source: &str) {
        // This analyzer accepts only struct HIR paired with struct source.
        let ItemKind::Struct(_, _, data) = item.kind else {
            return;
        };

        // Invalid syntax is already diagnosed by rustc.
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
        self.record_struct_group(cx, item, &container, &members);
    }

    /// Parses and analyzes one authored enum and its variant fields.
    fn analyze_enum(&mut self, cx: &LateContext<'_>, item: &Item<'_>, source: &str) {
        // This analyzer accepts only enum HIR paired with enum source.
        let ItemKind::Enum(_, _, definition) = item.kind else {
            return;
        };

        // Invalid syntax is already diagnosed by rustc.
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
                    is_authored_directionally: container.is_rename_all_directional,
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

        // Macro-generated items have no stable authored attribute representation.
        if item.span.from_expansion() {
            return;
        }

        // Unavailable source cannot support syntax-aware directive analysis.
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
            let exposure = Exposure::for_definition(cx, candidate.definition);
            if !active || (candidate.is_private_only && exposure.is_exported()) {
                continue;
            }
            candidate.emit(cx);
        }
    }
}
