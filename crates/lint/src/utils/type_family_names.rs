#![allow(unknown_lints)]
#![allow(
    misordered_inherent_impl_items,
    misordered_module_declarations,
    misordered_type_declarations
)]

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

use super::item_dependencies::item_dependencies;
use super::section_dividers::{DividerAnalyzer, DividerParticipant, DividerSection};

const REPORT_THRESHOLD: i32 = 7;
const SUGGESTION_THRESHOLD: i32 = 9;

// -----------------------------------------------------------------------------
// FamilyName: Diagnostics and analysis
// -----------------------------------------------------------------------------

/// One naming-family diagnostic with all supporting source labels.
pub(crate) struct FamilyNameFinding {
    pub(crate) span: Span,
    pub(crate) message: String,
    pub(crate) help: String,
    pub(crate) labels: Vec<(Span, String)>,
}

/// Finds naming families whose shared prefix reflects source organization instead of concepts.
pub(crate) struct FamilyNameAnalyzer {
    dividers: DividerAnalyzer,
}

impl FamilyNameAnalyzer {
    pub(crate) fn from_config() -> Self {
        Self {
            dividers: DividerAnalyzer::from_config(),
        }
    }

    pub(crate) fn analyze(
        &self,
        cx: &LateContext<'_>,
        module: &Mod<'_>,
        hir_id: HirId,
    ) -> Vec<FamilyNameFinding> {
        let analysis = self.dividers.analyze(cx, module, hir_id);
        let context = module_context(cx, module, hir_id);
        let dependencies = module_dependencies(cx, module);
        let occupied_names = module
            .item_ids
            .iter()
            .map(|item_id| cx.tcx.hir_item(*item_id))
            .filter_map(|item| nominal_name(cx, item.kind, item.owner_id.def_id))
            .collect::<HashSet<_>>();

        analysis
            .sections
            .iter()
            .filter_map(|section| {
                infer_family(section, context.as_deref(), &dependencies, &occupied_names)
            })
            .collect()
    }
}

// -----------------------------------------------------------------------------
// FamilyInference: Confidence-scored family inference
// -----------------------------------------------------------------------------

struct FamilyInference<'section> {
    score: i32,
    section: &'section DividerSection,
    renames: Vec<(&'section DividerParticipant, String)>,
    evidence: Vec<String>,
}

impl FamilyInference<'_> {
    fn into_finding(self, occupied_names: &HashSet<String>) -> Option<FamilyNameFinding> {
        if self.score < REPORT_THRESHOLD || self.renames.is_empty() {
            return None;
        }

        let has_collision = self.renames.iter().any(|(participant, replacement)| {
            replacement != &participant.name && occupied_names.contains(replacement)
        });
        let replacements = self
            .renames
            .iter()
            .map(|(participant, replacement)| format!("`{}` → `{replacement}`", participant.name))
            .collect::<Vec<_>>()
            .join(", ");
        let help = if exact_names_allowed(self.score, has_collision) {
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
            .map(|(participant, replacement)| {
                (
                    participant.span,
                    format!(
                        "`{}` obscures the inferred `{replacement}` role",
                        participant.name
                    ),
                )
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

fn exact_names_allowed(score: i32, has_collision: bool) -> bool {
    score >= SUGGESTION_THRESHOLD && !has_collision
}

fn infer_family(
    section: &DividerSection,
    context: Option<&str>,
    dependencies: &HashMap<LocalDefId, HashSet<LocalDefId>>,
    occupied_names: &HashSet<String>,
) -> Option<FamilyNameFinding> {
    let prefix = NameTokens::pascal(&section.prefix);
    let context_matches = prefix.len() >= 2
        && context
            .map(NameTokens::context)
            .is_some_and(|context| prefix.matches_context(&context));
    let mut affected = section
        .participants
        .iter()
        .filter_map(|participant| {
            NameTokens::pascal(&participant.name)
                .strip_prefix(&prefix)
                .filter(|remainder| !remainder.is_empty())
                .map(|remainder| (participant, remainder))
        })
        .collect::<Vec<_>>();
    if affected.is_empty() {
        return None;
    }

    let reverse_owner = reverse_owner_shape(&affected, dependencies);
    if !context_matches && reverse_owner.is_none() {
        return None;
    }

    let root = section
        .participants
        .iter()
        .find(|participant| participant.name == section.prefix);
    let exact_root = root.is_some();
    if single_type_is_semantically_rooted(&affected, root, dependencies) {
        return None;
    }
    let dependency_pair = affected
        .iter()
        .enumerate()
        .find_map(|(owner_index, (owner, _))| {
            affected
                .iter()
                .enumerate()
                .find_map(|(child_index, (child, _))| {
                    (owner_index != child_index
                        && dependencies
                            .get(&owner.def_id)
                            .is_some_and(|items| items.contains(&child.def_id)))
                    .then_some((owner_index, child_index))
                })
        });

    let competing_stems = dependency_stems(&affected, dependencies).len() > 1;
    let mut confidence = ConfidenceEvidence::default();
    let mut evidence = Vec::new();
    if context_matches {
        confidence.add(ConfidenceSignal::ContextMatch);
        evidence.push("the divider prefix repeats the enclosing module".to_owned());
    }
    if dependency_pair.is_some() {
        confidence.add(ConfidenceSignal::DependencyPair);
        evidence.push("compiler-resolved dependencies connect the helpers".to_owned());
    }
    if exact_root {
        confidence.add(ConfidenceSignal::ExactRoot);
        evidence.push("an exact root declaration already carries the context name".to_owned());
    }
    if affected.len() >= 2 {
        confidence.add(ConfidenceSignal::MultipleAffected);
    }
    if affected.iter().any(|(_, remainder)| remainder.len() == 1) {
        confidence.add(ConfidenceSignal::LosslessSingleToken);
        evidence.push("the redundant prefix can be removed without losing vocabulary".to_owned());
    }
    if reverse_owner.is_some() {
        confidence.add(ConfidenceSignal::ReverseOwner);
        evidence
            .push("the broader owner is qualified as though it were the nested type".to_owned());
    }
    if declarations_are_contiguous(section, &affected) {
        confidence.add(ConfidenceSignal::Contiguous);
    }
    if competing_stems {
        confidence.add(ConfidenceSignal::CompetingStem);
        evidence.push("multiple dependency owners provide competing family stems".to_owned());
    }

    let renames = infer_renames(
        &mut affected,
        reverse_owner,
        dependency_pair,
        &mut confidence,
    )?;

    FamilyInference {
        score: confidence.score(),
        section,
        renames,
        evidence,
    }
    .into_finding(occupied_names)
}

fn single_type_is_semantically_rooted(
    affected: &[(&DividerParticipant, NameTokens)],
    root: Option<&DividerParticipant>,
    dependencies: &HashMap<LocalDefId, HashSet<LocalDefId>>,
) -> bool {
    affected.len() == 1
        && root.is_some_and(|root| {
            dependencies
                .get(&affected[0].0.def_id)
                .is_some_and(|items| items.contains(&root.def_id))
                || dependencies
                    .get(&root.def_id)
                    .is_some_and(|items| items.contains(&affected[0].0.def_id))
        })
}

fn infer_renames<'participant>(
    affected: &mut Vec<(&'participant DividerParticipant, NameTokens)>,
    reverse_owner: Option<(usize, usize)>,
    dependency_pair: Option<(usize, usize)>,
    confidence: &mut ConfidenceEvidence,
) -> Option<Vec<(&'participant DividerParticipant, String)>> {
    let mut renames = Vec::new();
    if let Some((owner_index, child_index)) = reverse_owner {
        let (owner, owner_remainder) = &affected[owner_index];
        let (_, child_remainder) = &affected[child_index];
        if owner_remainder.last() == child_remainder.last() && owner_remainder.len() > 1 {
            renames.push((*owner, owner_remainder.join()));
        }
    } else if let Some((owner_index, child_index)) = dependency_pair {
        let owner_remainder = &affected[owner_index].1;
        if owner_remainder.len() > 1 {
            let stem = &owner_remainder.words[..owner_remainder.len() - 1];
            for (index, (participant, remainder)) in affected.iter().enumerate() {
                let replacement = if index == owner_index {
                    remainder.join()
                } else if index == child_index {
                    let mut words = stem.to_vec();
                    words.push(remainder.last()?.to_owned());
                    for _ in 1..remainder.len() {
                        confidence.add(ConfidenceSignal::DiscardedToken);
                    }
                    words.concat()
                } else {
                    remainder.join()
                };
                renames.push((*participant, replacement));
            }
        }
    }
    if renames.is_empty() {
        renames.extend(
            affected
                .drain(..)
                .map(|(participant, remainder)| (participant, remainder.join())),
        );
    }
    renames.retain(|(participant, replacement)| replacement != &participant.name);
    Some(renames)
}

fn dependency_stems(
    affected: &[(&DividerParticipant, NameTokens)],
    dependencies: &HashMap<LocalDefId, HashSet<LocalDefId>>,
) -> HashSet<String> {
    affected
        .iter()
        .filter(|(owner, owner_name)| {
            owner_name.len() > 1
                && affected.iter().any(|(child, _)| {
                    child.def_id != owner.def_id
                        && dependencies
                            .get(&owner.def_id)
                            .is_some_and(|items| items.contains(&child.def_id))
                })
        })
        .map(|(_, owner_name)| owner_name.words[..owner_name.len() - 1].concat())
        .collect()
}

fn reverse_owner_shape(
    affected: &[(&DividerParticipant, NameTokens)],
    dependencies: &HashMap<LocalDefId, HashSet<LocalDefId>>,
) -> Option<(usize, usize)> {
    affected
        .iter()
        .enumerate()
        .find_map(|(owner_index, (owner, owner_name))| {
            affected
                .iter()
                .enumerate()
                .find_map(|(child_index, (child, child_name))| {
                    (owner_name.len() > 1
                        && child_name.len() == 1
                        && owner_name.last() == child_name.last()
                        && dependencies
                            .get(&owner.def_id)
                            .is_some_and(|items| items.contains(&child.def_id)))
                    .then_some((owner_index, child_index))
                })
        })
}

fn declarations_are_contiguous(
    section: &DividerSection,
    affected: &[(&DividerParticipant, NameTokens)],
) -> bool {
    let indices = affected
        .iter()
        .filter_map(|(affected, _)| {
            section
                .participants
                .iter()
                .position(|participant| participant.def_id == affected.def_id)
        })
        .collect::<Vec<_>>();
    indices.windows(2).all(|window| window[1] == window[0] + 1)
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
            words: pascal_words(value),
        }
    }

    fn context(value: &str) -> Self {
        let words = if value.contains('_') {
            value
                .split('_')
                .filter(|word| !word.is_empty())
                .map(|word| {
                    let mut characters = word.chars();
                    characters.next().map_or_else(String::new, |first| {
                        first.to_uppercase().chain(characters).collect()
                    })
                })
                .collect()
        } else {
            pascal_words(value)
        };
        Self { words }
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
                if left.eq_ignore_ascii_case(right) {
                    return true;
                }
                index + 1 == self.words.len()
                    && left
                        .strip_suffix('s')
                        .is_some_and(|left| left.eq_ignore_ascii_case(right))
                    || right
                        .strip_suffix('s')
                        .is_some_and(|right| left.eq_ignore_ascii_case(right))
            })
    }
}

fn pascal_words(value: &str) -> Vec<String> {
    let characters = value.char_indices().collect::<Vec<_>>();
    if characters.is_empty() {
        return Vec::new();
    }
    let mut starts = vec![0];
    for index in 1..characters.len() {
        let (_, previous) = characters[index - 1];
        let (byte, current) = characters[index];
        let next_is_lower = characters
            .get(index + 1)
            .is_some_and(|(_, next)| next.is_lowercase());
        if current.is_uppercase()
            && (previous.is_lowercase() || previous.is_numeric() || next_is_lower)
        {
            starts.push(byte);
        }
    }
    starts.push(value.len());
    starts
        .windows(2)
        .map(|window| value[window[0]..window[1]].to_owned())
        .collect()
}

fn module_context(cx: &LateContext<'_>, module: &Mod<'_>, hir_id: HirId) -> Option<String> {
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

fn module_dependencies(
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

fn nominal_name(cx: &LateContext<'_>, kind: ItemKind<'_>, def_id: LocalDefId) -> Option<String> {
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

#[cfg(test)]
mod tests {
    use super::{
        ConfidenceEvidence, ConfidenceSignal, NameTokens, REPORT_THRESHOLD, SUGGESTION_THRESHOLD,
        exact_names_allowed, pascal_words,
    };

    #[test]
    fn tokenizes_pascal_case_and_acronyms() {
        assert_eq!(
            pascal_words("HTTPServerConfig"),
            ["HTTP", "Server", "Config"]
        );
        assert_eq!(pascal_words("MigrationEdits"), ["Migration", "Edits"]);
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
        assert_eq!(clear.score(), SUGGESTION_THRESHOLD);

        let mut ambiguous = clear;
        ambiguous.add(ConfidenceSignal::CompetingStem);
        assert!(ambiguous.score() < REPORT_THRESHOLD);
    }

    #[test]
    fn suppresses_exact_names_when_a_candidate_collides() {
        assert!(exact_names_allowed(SUGGESTION_THRESHOLD, false));
        assert!(!exact_names_allowed(SUGGESTION_THRESHOLD, true));
        assert!(!exact_names_allowed(SUGGESTION_THRESHOLD - 1, false));
    }
}
