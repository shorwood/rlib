extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_span;

use std::collections::{HashMap, HashSet};

use rustc_hir::def_id::LocalDefId;
use rustc_hir::{HirId, ItemKind, Mod, Node};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty;
use rustc_span::Span;

use super::identifier_case::{identifier_pascal_words, identifier_words};
use super::item_dependencies::item_dependencies;
use super::section_analysis::SectionAnalyzer;
use super::source_organization::{SectionGroup, SectionParticipant};

// -----------------------------------------------------------------------------
// Threshold: Inference reporting thresholds
// -----------------------------------------------------------------------------

const THRESHOLD_REPORT: i32 = 7;
const THRESHOLD_SUGGESTION: i32 = 9;

// -----------------------------------------------------------------------------
// FamilyName: Diagnostics and analysis
// -----------------------------------------------------------------------------

/// One naming-family diagnostic with all supporting source labels.
pub(crate) struct FamilyNameLabel {
    pub(crate) span: Span,
    pub(crate) message: String,
}

/// One naming-family diagnostic with all supporting source labels.
pub(crate) struct FamilyNameFinding {
    pub(crate) span: Span,
    pub(crate) message: String,
    pub(crate) help: String,
    pub(crate) labels: Vec<FamilyNameLabel>,
}

/// Finds naming families whose shared prefix reflects source organization instead of concepts.
pub(crate) struct FamilyNameAnalyzer {
    dividers: SectionAnalyzer,
}

impl FamilyNameAnalyzer {
    pub(crate) fn from_config() -> Self {
        Self {
            dividers: SectionAnalyzer::from_config(),
        }
    }

    pub(crate) fn analyze(
        &self,
        cx: &LateContext<'_>,
        module: &Mod<'_>,
        hir_id: HirId,
    ) -> Vec<FamilyNameFinding> {
        let analysis = self.dividers.analyze(cx, module, hir_id);
        let module_analysis = ModuleNamingAnalysis::collect(cx, module, hir_id);
        let mut findings = analysis
            .sections
            .iter()
            .filter_map(|section| {
                FamilyCandidateSet::collect(section, &module_analysis.dependencies).infer(
                    module_analysis.context.as_deref(),
                    &module_analysis.occupied_names,
                )
            })
            .collect::<Vec<_>>();

        let covered = analysis
            .sections
            .iter()
            .flat_map(|section| &section.participants)
            .map(|participant| participant.def_id)
            .collect::<HashSet<_>>();
        let unsectioned = module_analysis
            .participants
            .into_iter()
            .filter(|participant| !covered.contains(&participant.def_id))
            .collect::<Vec<_>>();
        if !unsectioned.is_empty()
            && let Some(prefix) = module_analysis.context.as_deref().map(NameTokens::context)
            && !prefix.is_empty()
        {
            let section = SectionGroup {
                ordinal: 0,
                prefix: prefix.join(),
                span: module.spans.inner_span,
                participants: unsectioned,
            };
            if let Some(finding) =
                FamilyCandidateSet::collect(&section, &module_analysis.dependencies).infer(
                    module_analysis.context.as_deref(),
                    &module_analysis.occupied_names,
                )
            {
                findings.push(finding);
            }
        }
        findings
    }
}

// -----------------------------------------------------------------------------
// FamilyInference: Confidence scored family inference
// -----------------------------------------------------------------------------

struct FamilyInferenceRename<'section> {
    participant: &'section SectionParticipant,
    replacement: String,
}

struct FamilyInference<'section> {
    score: i32,
    section: &'section SectionGroup,
    renames: Vec<FamilyInferenceRename<'section>>,
    evidence: Vec<String>,
}

impl FamilyInference<'_> {
    fn into_finding(self, occupied_names: &HashSet<String>) -> Option<FamilyNameFinding> {
        if self.score < THRESHOLD_REPORT || self.renames.is_empty() {
            return None;
        }

        let has_collision = self.renames.iter().any(|rename| {
            rename.replacement != rename.participant.name
                && occupied_names.contains(&rename.replacement)
        });
        let replacements = self
            .renames
            .iter()
            .map(|rename| format!("`{}` → `{}`", rename.participant.name, rename.replacement))
            .collect::<Vec<_>>()
            .join(", ");
        let help = if ConfidenceEvidence::allows_exact_names(self.score, has_collision) {
            format!(
                "prefer the concept-first names {replacements}; rename before creating additional sections"
            )
        } else if has_collision {
            "the concise candidate name is already occupied; choose another concept-first name instead of adding another section".to_owned()
        } else {
            "reconsider the family vocabulary before adding another section; the evidence is not strong enough to prescribe exact names".to_owned()
        };
        let evidence = self.evidence.join("; ");
        let labels = self
            .renames
            .iter()
            .map(|rename| FamilyNameLabel {
                span: rename.participant.span,
                message: format!(
                    "`{}` obscures the inferred `{}` role",
                    rename.participant.name, rename.replacement
                ),
            })
            .collect();

        Some(FamilyNameFinding {
            span: self.section.span,
            message: format!(
                "type names in the `{}` section repeat organizational context",
                self.section.prefix
            ),
            help: format!("{help} ({evidence})"),
            labels,
        })
    }
}

// -----------------------------------------------------------------------------
// Confidence: Fixed inference evidence weights
// -----------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum ConfidenceSignal {
    ContextMatch,
    DependencyPair,
    ExactRoot,
    MultipleAffected,
    LosslessSingleToken,
    ReverseOwner,
    Contiguous,
    DiscardedToken,
    CompetingStem,
}

#[derive(Default)]
struct ConfidenceEvidence(Vec<ConfidenceSignal>);

impl ConfidenceEvidence {
    fn allows_exact_names(score: i32, has_collision: bool) -> bool {
        score >= THRESHOLD_SUGGESTION && !has_collision
    }

    fn add(&mut self, signal: ConfidenceSignal) {
        self.0.push(signal);
    }

    fn score(&self) -> i32 {
        self.0
            .iter()
            .map(|signal| match signal {
                ConfidenceSignal::ContextMatch => 4,
                ConfidenceSignal::DependencyPair => 3,
                ConfidenceSignal::ExactRoot
                | ConfidenceSignal::MultipleAffected
                | ConfidenceSignal::LosslessSingleToken
                | ConfidenceSignal::ReverseOwner => 2,
                ConfidenceSignal::Contiguous => 1,
                ConfidenceSignal::DiscardedToken | ConfidenceSignal::CompetingStem => -3,
            })
            .sum()
    }
}

// -----------------------------------------------------------------------------
// FamilyCandidate: Declarations considered by family inference
// -----------------------------------------------------------------------------

struct FamilyCandidate<'section> {
    participant: &'section SectionParticipant,
    name: NameTokens,
}

#[derive(Clone, Copy)]
struct FamilyCandidateRelationship {
    owner_index: usize,
    child_index: usize,
}

struct FamilyCandidateSet<'section, 'analysis> {
    section: &'section SectionGroup,
    affected: Vec<FamilyCandidate<'section>>,
    dependencies: &'analysis HashMap<LocalDefId, HashSet<LocalDefId>>,
}

impl<'section, 'analysis> FamilyCandidateSet<'section, 'analysis> {
    fn collect(
        section: &'section SectionGroup,
        dependencies: &'analysis HashMap<LocalDefId, HashSet<LocalDefId>>,
    ) -> Self {
        let prefix = NameTokens::pascal(&section.prefix);
        let affected = section
            .participants
            .iter()
            .filter(|participant| participant.is_nominal)
            .filter_map(|participant| {
                NameTokens::pascal(&participant.name)
                    .strip_prefix(&prefix)
                    .filter(|name| !name.is_empty())
                    .map(|name| FamilyCandidate { participant, name })
            })
            .collect();
        Self {
            section,
            affected,
            dependencies,
        }
    }

    fn single_participant_is_semantically_rooted(&self, root: Option<&SectionParticipant>) -> bool {
        self.affected.len() == 1
            && root.is_some_and(|root| {
                let affected = self.affected[0].participant;
                self.dependencies
                    .get(&affected.def_id)
                    .is_some_and(|items| items.contains(&root.def_id))
                    || self
                        .dependencies
                        .get(&root.def_id)
                        .is_some_and(|items| items.contains(&affected.def_id))
            })
    }

    fn dependency_relationship(&self) -> Option<FamilyCandidateRelationship> {
        self.affected
            .iter()
            .enumerate()
            .find_map(|(owner_index, owner)| {
                self.affected
                    .iter()
                    .enumerate()
                    .find_map(|(child_index, child)| {
                        (owner_index != child_index
                            && self
                                .dependencies
                                .get(&owner.participant.def_id)
                                .is_some_and(|items| items.contains(&child.participant.def_id)))
                        .then_some(FamilyCandidateRelationship {
                            owner_index,
                            child_index,
                        })
                    })
            })
    }

    fn dependency_stems(&self) -> HashSet<String> {
        self.affected
            .iter()
            .filter(|owner| {
                owner.name.len() > 1
                    && self.affected.iter().any(|child| {
                        child.participant.def_id != owner.participant.def_id
                            && self
                                .dependencies
                                .get(&owner.participant.def_id)
                                .is_some_and(|items| items.contains(&child.participant.def_id))
                    })
            })
            .map(|owner| owner.name.words[..owner.name.len() - 1].concat())
            .collect()
    }

    fn reverse_owner_relationship(&self) -> Option<FamilyCandidateRelationship> {
        self.affected
            .iter()
            .enumerate()
            .find_map(|(owner_index, owner)| {
                self.affected
                    .iter()
                    .enumerate()
                    .find_map(|(child_index, child)| {
                        (owner.name.len() > 1
                            && child.name.len() == 1
                            && owner.name.last() == child.name.last()
                            && self
                                .dependencies
                                .get(&owner.participant.def_id)
                                .is_some_and(|items| items.contains(&child.participant.def_id)))
                        .then_some(FamilyCandidateRelationship {
                            owner_index,
                            child_index,
                        })
                    })
            })
    }

    fn declarations_are_contiguous(&self) -> bool {
        let indices = self
            .affected
            .iter()
            .filter_map(|affected| {
                self.section
                    .participants
                    .iter()
                    .position(|participant| participant.def_id == affected.participant.def_id)
            })
            .collect::<Vec<_>>();
        indices.windows(2).all(|window| window[1] == window[0] + 1)
    }

    fn renames(
        &mut self,
        reverse_owner: Option<FamilyCandidateRelationship>,
        dependency_relationship: Option<FamilyCandidateRelationship>,
        confidence: &mut ConfidenceEvidence,
    ) -> Option<Vec<FamilyInferenceRename<'section>>> {
        let mut renames = Vec::new();
        if let Some(relationship) = reverse_owner {
            let owner = &self.affected[relationship.owner_index];
            let child = &self.affected[relationship.child_index];
            if owner.name.last() == child.name.last() && owner.name.len() > 1 {
                renames.push(FamilyInferenceRename {
                    participant: owner.participant,
                    replacement: owner.name.join(),
                });
            }
        } else if let Some(relationship) = dependency_relationship {
            let owner_name = &self.affected[relationship.owner_index].name;
            if owner_name.len() > 1 {
                let stem = &owner_name.words[..owner_name.len() - 1];
                for (index, affected) in self.affected.iter().enumerate() {
                    let replacement = if index == relationship.owner_index {
                        affected.name.join()
                    } else if index == relationship.child_index {
                        let mut words = stem.to_vec();
                        words.push(affected.name.last()?.to_owned());
                        for _ in 1..affected.name.len() {
                            confidence.add(ConfidenceSignal::DiscardedToken);
                        }
                        words.concat()
                    } else {
                        affected.name.join()
                    };
                    renames.push(FamilyInferenceRename {
                        participant: affected.participant,
                        replacement,
                    });
                }
            }
        }
        if renames.is_empty() {
            renames.extend(
                self.affected
                    .drain(..)
                    .map(|affected| FamilyInferenceRename {
                        participant: affected.participant,
                        replacement: affected.name.join(),
                    }),
            );
        }
        renames.retain(|rename| rename.replacement != rename.participant.name);
        Some(renames)
    }
    fn infer(
        mut self,
        context: Option<&str>,
        occupied_names: &HashSet<String>,
    ) -> Option<FamilyNameFinding> {
        if self.affected.is_empty() {
            return None;
        }
        let prefix = NameTokens::pascal(&self.section.prefix);
        let context_matches = prefix.len() >= 2
            && context
                .map(NameTokens::context)
                .is_some_and(|context| prefix.matches_context(&context));
        let reverse_owner = self.reverse_owner_relationship();
        if !context_matches && reverse_owner.is_none() {
            return None;
        }

        let root = self
            .section
            .participants
            .iter()
            .find(|participant| participant.name == self.section.prefix);
        let exact_root = root.is_some();
        if self.single_participant_is_semantically_rooted(root) {
            return None;
        }
        let dependency_relationship = self.dependency_relationship();
        let competing_stems = self.dependency_stems().len() > 1;
        let mut confidence = ConfidenceEvidence::default();
        let mut evidence = Vec::new();
        if context_matches {
            confidence.add(ConfidenceSignal::ContextMatch);
            evidence.push("the divider prefix repeats the enclosing module".to_owned());
        }
        if dependency_relationship.is_some() {
            confidence.add(ConfidenceSignal::DependencyPair);
            evidence.push("compiler-resolved dependencies connect the helpers".to_owned());
        }
        if exact_root {
            confidence.add(ConfidenceSignal::ExactRoot);
            evidence.push("an exact root declaration already carries the context name".to_owned());
        }
        if self.affected.len() >= 2 {
            confidence.add(ConfidenceSignal::MultipleAffected);
        }
        if self
            .affected
            .iter()
            .any(|affected| affected.name.len() == 1)
        {
            confidence.add(ConfidenceSignal::LosslessSingleToken);
            evidence
                .push("the redundant prefix can be removed without losing vocabulary".to_owned());
        }
        if reverse_owner.is_some() {
            confidence.add(ConfidenceSignal::ReverseOwner);
            evidence.push(
                "the broader owner is qualified as though it were the nested type".to_owned(),
            );
        }
        if self.declarations_are_contiguous() {
            confidence.add(ConfidenceSignal::Contiguous);
            confidence.add(ConfidenceSignal::Contiguous);
        }
        if competing_stems {
            confidence.add(ConfidenceSignal::CompetingStem);
            evidence.push("multiple dependency owners provide competing family stems".to_owned());
        }

        let renames = self.renames(reverse_owner, dependency_relationship, &mut confidence)?;
        FamilyInference {
            score: confidence.score(),
            section: self.section,
            renames,
            evidence,
        }
        .into_finding(occupied_names)
    }
}

// -----------------------------------------------------------------------------
// NameTokens: Rust identifier word structure
// -----------------------------------------------------------------------------

#[derive(Clone, Debug, Eq, PartialEq)]
struct NameTokens {
    words: Vec<String>,
}

impl NameTokens {
    fn pascal(value: &str) -> Self {
        Self {
            words: identifier_pascal_words(value),
        }
    }

    fn context(value: &str) -> Self {
        Self {
            words: identifier_words(value),
        }
    }

    fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    fn len(&self) -> usize {
        self.words.len()
    }

    fn last(&self) -> Option<&str> {
        self.words.last().map(String::as_str)
    }

    fn join(&self) -> String {
        self.words.concat()
    }

    fn strip_prefix(&self, prefix: &Self) -> Option<Self> {
        self.words.starts_with(&prefix.words).then(|| Self {
            words: self.words[prefix.len()..].to_vec(),
        })
    }

    fn matches_context(&self, context: &Self) -> bool {
        if self.words.len() != context.words.len() {
            return false;
        }
        self.words
            .iter()
            .zip(&context.words)
            .enumerate()
            .all(|(index, (left, right))| {
                if left == right {
                    return true;
                }
                index + 1 == self.words.len()
                    && left.strip_suffix('s').is_some_and(|left| left == right)
                    || right.strip_suffix('s').is_some_and(|right| left == right)
            })
    }
}

// -----------------------------------------------------------------------------
// ModuleNamingAnalysis: Module-level semantic context
// -----------------------------------------------------------------------------

struct ModuleNamingAnalysis {
    context: Option<String>,
    dependencies: HashMap<LocalDefId, HashSet<LocalDefId>>,
    participants: Vec<SectionParticipant>,
    occupied_names: HashSet<String>,
}

impl ModuleNamingAnalysis {
    fn collect(cx: &LateContext<'_>, module: &Mod<'_>, hir_id: HirId) -> Self {
        let participants = module
            .item_ids
            .iter()
            .map(|item_id| cx.tcx.hir_item(*item_id))
            .filter(|item| !item.span.from_expansion())
            .filter_map(|item| {
                Self::nominal_name(cx, item.kind, item.owner_id.def_id).map(|name| {
                    SectionParticipant {
                        def_id: item.owner_id.def_id,
                        name,
                        span: item.span,
                        is_nominal: true,
                    }
                })
            })
            .collect::<Vec<_>>();
        let occupied_names = participants
            .iter()
            .map(|participant| participant.name.clone())
            .collect();
        Self {
            context: Self::context(cx, module, hir_id),
            dependencies: Self::dependencies(cx, module),
            participants,
            occupied_names,
        }
    }

    fn context(cx: &LateContext<'_>, module: &Mod<'_>, hir_id: HirId) -> Option<String> {
        for node in [cx.tcx.hir_node(hir_id), cx.tcx.parent_hir_node(hir_id)] {
            if let Node::Item(item) = node
                && let Some(identifier) = item.kind.ident()
            {
                return Some(identifier.name.to_string());
            }
        }
        cx.sess()
            .source_map()
            .span_to_filename(module.spans.inner_span)
            .into_local_path()
            .and_then(|path| {
                path.file_stem()
                    .map(|stem| stem.to_string_lossy().into_owned())
            })
    }

    fn dependencies(
        cx: &LateContext<'_>,
        module: &Mod<'_>,
    ) -> HashMap<LocalDefId, HashSet<LocalDefId>> {
        let mut dependencies = HashMap::<LocalDefId, HashSet<LocalDefId>>::new();
        for item_id in module.item_ids {
            let item = cx.tcx.hir_item(*item_id);
            match item.kind {
                ItemKind::Struct(..)
                | ItemKind::Enum(..)
                | ItemKind::Union(..)
                | ItemKind::TyAlias(..)
                | ItemKind::Trait(..)
                | ItemKind::TraitAlias(..) => {
                    dependencies
                        .entry(item.owner_id.def_id)
                        .or_default()
                        .extend(item_dependencies(cx.tcx, item));
                }
                ItemKind::Impl(_) => {
                    let self_type = cx.tcx.type_of(item.owner_id).instantiate_identity();
                    if let ty::Adt(definition, _) = self_type.kind()
                        && let Some(definition) = definition.did().as_local()
                    {
                        dependencies
                            .entry(definition)
                            .or_default()
                            .extend(item_dependencies(cx.tcx, item));
                    }
                }
                _ => {}
            }
        }
        dependencies
    }

    fn nominal_name(
        cx: &LateContext<'_>,
        kind: ItemKind<'_>,
        def_id: LocalDefId,
    ) -> Option<String> {
        matches!(
            kind,
            ItemKind::Struct(..)
                | ItemKind::Enum(..)
                | ItemKind::Union(..)
                | ItemKind::TyAlias(..)
                | ItemKind::Trait(..)
                | ItemKind::TraitAlias(..)
        )
        .then(|| cx.tcx.item_name(def_id.to_def_id()).to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ConfidenceEvidence, ConfidenceSignal, NameTokens, THRESHOLD_REPORT, THRESHOLD_SUGGESTION,
    };
    use crate::utils::identifier_case::identifier_pascal_words;

    #[test]
    fn tokenizes_pascal_case_and_acronyms() {
        assert_eq!(
            identifier_pascal_words("HttpServerConfig"),
            ["Http", "Server", "Config"]
        );
        assert_eq!(
            identifier_pascal_words("MigrationEdits"),
            ["Migration", "Edits"]
        );
    }

    #[test]
    fn matches_snake_case_context_and_plural_variants() {
        assert!(
            NameTokens::pascal("MethodLikeFreeFunctions")
                .matches_context(&NameTokens::context("method_like_free_functions"))
        );
        assert!(
            NameTokens::pascal("SectionDivider")
                .matches_context(&NameTokens::context("section_dividers"))
        );
    }

    #[test]
    fn applies_fixed_confidence_weights_and_ambiguity_penalties() {
        let clear = ConfidenceEvidence(vec![
            ConfidenceSignal::ContextMatch,
            ConfidenceSignal::DependencyPair,
            ConfidenceSignal::ExactRoot,
            ConfidenceSignal::MultipleAffected,
            ConfidenceSignal::Contiguous,
            ConfidenceSignal::DiscardedToken,
        ]);
        assert_eq!(clear.score(), THRESHOLD_SUGGESTION);

        let mut ambiguous = clear;
        ambiguous.add(ConfidenceSignal::CompetingStem);
        assert!(ambiguous.score() < THRESHOLD_REPORT);
    }

    #[test]
    fn suppresses_exact_names_when_a_candidate_collides() {
        assert!(ConfidenceEvidence::allows_exact_names(
            THRESHOLD_SUGGESTION,
            false
        ));
        assert!(!ConfidenceEvidence::allows_exact_names(
            THRESHOLD_SUGGESTION,
            true
        ));
        assert!(!ConfidenceEvidence::allows_exact_names(
            THRESHOLD_SUGGESTION - 1,
            false
        ));
    }
}
