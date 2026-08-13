extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_span;

use std::collections::{HashMap, HashSet};

use rustc_hir::def_id::LocalDefId;
use rustc_hir::{HirId, Item, ItemKind, Mod, Node};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty;
use rustc_span::Span;

use super::identifier_case;
use super::item_dependencies::DependenciesExt;
use super::section_analysis::{SectionAnalyzer, SectionGroup, SectionParticipant};
use super::source_provenance::ItemProvenanceExt;

// -----------------------------------------------------------------------------
// Threshold: Inference reporting thresholds
// -----------------------------------------------------------------------------

/// Minimum confidence score at which the analyzer reports an incoherent family.
const THRESHOLD_REPORT: i32 = 7;
/// Minimum confidence score at which exact replacement names are trustworthy.
const THRESHOLD_SUGGESTION: i32 = 9;

// -----------------------------------------------------------------------------
// FamilyName: Diagnostics and analysis
// -----------------------------------------------------------------------------

/// One supporting source label attached to a naming-family diagnostic.
pub struct FamilyNameLabel {
    /// Declaration source range annotated by the supporting label.
    pub(crate) span: Span,
    /// Explanation of the inferred role obscured by the declaration name.
    pub(crate) message: String,
}

/// One naming-family diagnostic with all supporting source labels.
pub struct FamilyNameFinding {
    /// Section divider source range used as the primary diagnostic site.
    pub(crate) span: Span,
    /// Summary of the naming-family problem.
    pub(crate) message: String,
    /// Confidence-aware remediation and evidence.
    pub(crate) help: String,
    /// Declaration-specific explanations for proposed renames.
    pub(crate) labels: Vec<FamilyNameLabel>,
}

/// Finds naming families whose shared prefix reflects source organization instead of concepts.
pub struct FamilyNameAnalyzer {
    /// Shared section parser that supplies authored declaration families.
    sections: SectionAnalyzer,
}

impl FamilyNameAnalyzer {
    /// Builds a family analyzer from the configured section-divider policy.
    pub(crate) fn from_config() -> Self {
        Self {
            sections: SectionAnalyzer::from_config(),
        }
    }

    /// Infers incoherent family names in one source module.
    pub(crate) fn analyze(
        &self,
        cx: &LateContext<'_>,
        module: &Mod<'_>,
        hir_id: HirId,
    ) -> Vec<FamilyNameFinding> {
        // Analyze authored sections against the module's complete naming context.
        let analysis = self.sections.analyze(cx, module, hir_id);
        let module_analysis = ModuleNamingAnalysis::collect(cx, module, hir_id);

        // Infer findings independently for every authored section.
        let section_candidates = analysis.sections.iter().filter_map(|section| {
            FamilyCandidateSet::collect(section, &module_analysis.dependencies).infer(
                module_analysis.context.as_deref(),
                &module_analysis.occupied_names,
            )
        });

        // Retain authored findings before considering unsectioned declarations.
        let mut findings = section_candidates.collect::<Vec<_>>();

        // Infer one synthetic family from declarations outside authored sections.
        let section_participants = analysis
            .sections
            .iter()
            .flat_map(|section| &section.participants);

        // Exclude declarations already represented by an authored section.
        let covered = section_participants
            .map(|participant| participant.def_id)
            .collect::<HashSet<_>>();

        // Collect the remaining declarations for contextual inference.
        let unsectioned_participants = module_analysis.participants.into_iter();
        let unsectioned = unsectioned_participants
            .filter(|participant| !covered.contains(&participant.def_id))
            .collect::<Vec<_>>();

        // Infer a contextual synthetic section only when unsectioned declarations remain.
        if !unsectioned.is_empty()
            && let Some(prefix) = module_analysis.context.as_deref().map(NameTokens::context)
            && !prefix.is_empty()
        {
            // Represent the unsectioned declarations under their enclosing context.
            let section = SectionGroup {
                ordinal: 0,
                prefix: prefix.join(),
                span: module.spans.inner_span,
                participants: unsectioned,
            };

            // Append a finding only when the synthetic family has actionable evidence.
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

/// Proposed concept-first name for one section participant.
struct FamilyInferenceRename<'section> {
    /// Declaration whose name should change.
    participant: &'section SectionParticipant,
    /// Inferred replacement without redundant organizational context.
    replacement: String,
}

/// Scored evidence and proposed renames for one declaration family.
struct FamilyInference<'section> {
    /// Sum of fixed confidence weights gathered during inference.
    score: i32,
    /// Authored section from which the family was inferred.
    section: &'section SectionGroup,
    /// Declarations for which concept-first replacements were inferred.
    renames: Vec<FamilyInferenceRename<'section>>,
    /// Human-readable evidence contributing to the score.
    evidence: Vec<String>,
}

impl FamilyInference<'_> {
    /// Converts sufficiently strong, actionable inference into a diagnostic.
    fn into_finding(self, occupied_names: &HashSet<String>) -> Option<FamilyNameFinding> {
        // Reject weak or non-actionable inferences before rendering diagnostic details.
        if self.score < THRESHOLD_REPORT || self.renames.is_empty() {
            return None;
        }

        let availability = ConfidenceNameAvailability::for_renames(&self.renames, occupied_names);

        // Render every inferred replacement for the diagnostic guidance.
        let rendered_renames = self
            .renames
            .iter()
            .map(|rename| format!("`{}` → `{}`", rename.participant.name, rename.replacement));
        let replacements = rendered_renames.collect::<Vec<_>>().join(", ");

        // Tailor naming guidance to confidence and namespace collisions.
        let help = if ConfidenceEvidence::allows_exact_names(self.score, availability) {
            format!(
                "prefer the concept-first names {replacements}; rename before creating additional sections"
            )
        } else if matches!(availability, ConfidenceNameAvailability::Occupied) {
            "the concise candidate name is already occupied; choose another concept-first name instead of adding another section".to_owned()
        } else {
            "reconsider the family vocabulary before adding another section; the evidence is not strong enough to prescribe exact names".to_owned()
        };

        // Attach the supporting evidence and one focused label per proposed rename.
        let evidence = self.evidence.join("; ");

        // Explain the obscured role at each affected declaration.
        let rename_labels = self.renames.iter().map(|rename| FamilyNameLabel {
            span: rename.participant.span,
            message: format!(
                "`{}` obscures the inferred `{}` role",
                rename.participant.name, rename.replacement
            ),
        });

        // Materialize labels after their messages have captured every inferred role.
        let labels = rename_labels.collect();

        // Describe the affected family once before assembling the final finding.
        let message = format!(
            "type names in the `{}` section repeat organizational context",
            self.section.prefix
        );

        // Assemble the family-level diagnostic from its rendered evidence.
        Some(FamilyNameFinding {
            span: self.section.span,
            message,
            help: format!("{help} ({evidence})"),
            labels,
        })
    }
}

// -----------------------------------------------------------------------------
// Confidence: Fixed inference evidence weights
// -----------------------------------------------------------------------------
/// Fixed evidence categories used by family-name confidence scoring.
#[derive(Clone, Copy)]
enum ConfidenceSignal {
    /// The section prefix repeats the containing module's words.
    ContextMatch,
    /// Compiler-resolved dependencies connect two family members.
    DependencyPair,
    /// A declaration already uses the exact section prefix.
    ExactRoot,
    /// More than one declaration is affected by prefix removal.
    MultipleAffected,
    /// At least one affected name retains one complete role token.
    LosslessSingleToken,
    /// A broader owner depends on a shorter type with the same role suffix.
    ReverseOwner,
    /// Affected declarations are adjacent in source order.
    Contiguous,
    /// Constructing a coherent name would discard authored vocabulary.
    DiscardedToken,
    /// Multiple dependency owners imply incompatible family stems.
    CompetingStem,
}
/// Namespace availability of an inferred exact replacement.
#[derive(Clone, Copy)]
enum ConfidenceNameAvailability {
    /// Every inferred replacement is available.
    Available,
    /// At least one inferred replacement collides with an existing declaration.
    Occupied,
}

impl ConfidenceNameAvailability {
    /// Resolves whether every inferred replacement remains free in the module namespace.
    fn for_renames(
        renames: &[FamilyInferenceRename<'_>],
        occupied_names: &HashSet<String>,
    ) -> Self {
        let has_collision = renames.iter().any(|rename| {
            rename.replacement != rename.participant.name
                && occupied_names.contains(&rename.replacement)
        });
        if has_collision {
            Self::Occupied
        } else {
            Self::Available
        }
    }
}
/// Ordered confidence signals collected while evaluating one family.
#[derive(Default)]
struct ConfidenceEvidence(
    /// Signals retained individually so repeated penalties remain meaningful.
    Vec<ConfidenceSignal>,
);

impl ConfidenceEvidence {
    /// Returns whether confidence and namespace occupancy permit exact rename advice.
    const fn allows_exact_names(score: i32, availability: ConfidenceNameAvailability) -> bool {
        score >= THRESHOLD_SUGGESTION
            && matches!(availability, ConfidenceNameAvailability::Available)
    }

    /// Records one confidence signal.
    fn add(&mut self, signal: ConfidenceSignal) {
        self.0.push(signal);
    }

    /// Penalizes each role token an inferred replacement would discard.
    fn discard_tokens(&mut self, count: usize) {
        for _ in 0..count {
            self.add(ConfidenceSignal::DiscardedToken);
        }
    }

    /// Sums the fixed weights of all collected signals.
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

/// Nominal declaration whose redundant section prefix can be removed.
struct FamilyCandidate<'section> {
    /// Authored declaration represented by this `candidate`.
    participant: &'section SectionParticipant,
    /// Role-bearing name tokens left after removing the section prefix.
    name: NameTokens,
}
/// Directed dependency relationship between two affected candidates.
#[derive(Clone, Copy)]
struct FamilyCandidateRelationship {
    /// Index of the declaration that owns or refers to the child.
    owner_index: usize,
    /// Index of the referenced child declaration.
    child_index: usize,
}

/// Candidate declarations and semantic evidence for one authored section.
struct FamilyCandidateSet<'section, 'analysis> {
    /// Section whose nominal declarations are being evaluated.
    section: &'section SectionGroup,
    /// Declarations retaining a nonempty role after prefix removal.
    affected: Vec<FamilyCandidate<'section>>,
    /// Compiler-resolved dependencies keyed by local declaration.
    dependencies: &'analysis HashMap<LocalDefId, HashSet<LocalDefId>>,
}

impl<'section, 'analysis> FamilyCandidateSet<'section, 'analysis> {
    /// Collects prefixed nominal declarations and strips their organizational prefix.
    fn collect(
        section: &'section SectionGroup,
        dependencies: &'analysis HashMap<LocalDefId, HashSet<LocalDefId>>,
    ) -> Self {
        // Tokenize the authored family prefix and retain only nominal participants.
        let prefix = NameTokens::pascal(&section.prefix);
        let nominal = section
            .participants
            .iter()
            .filter(|participant| participant.is_nominal);

        // Strip the section prefix from every nominal declaration that retains a role name.
        let candidates = nominal.filter_map(|participant| {
            NameTokens::pascal(&participant.name)
                .strip_prefix(&prefix)
                .filter(|name| !name.is_empty())
                .map(|name| FamilyCandidate { participant, name })
        });
        let affected = candidates.collect();

        // Retain dependency evidence for the later confidence-scoring pass.
        Self {
            section,
            affected,
            dependencies,
        }
    }

    /// Detects a lone helper whose relationship to an exact root justifies its prefix.
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

    /// Finds the first compiler-resolved dependency between affected candidates.
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

    /// Returns whether `owner` refers to another affected family member.
    fn has_dependent_family_member(&self, owner: &FamilyCandidate<'_>) -> bool {
        owner.name.len() > 1
            && self.affected.iter().any(|child| {
                child.participant.def_id != owner.participant.def_id
                    && self
                        .dependencies
                        .get(&owner.participant.def_id)
                        .is_some_and(|items| items.contains(&child.participant.def_id))
            })
    }

    /// Returns distinct multi-token stems offered by dependency-owning candidates.
    fn dependency_stems(&self) -> HashSet<String> {
        let owners = self
            .affected
            .iter()
            .filter(|owner| self.has_dependent_family_member(owner));
        owners
            .map(|owner| owner.name.words[..owner.name.len() - 1].concat())
            .collect()
    }

    /// Finds a broad owner qualified as though it were its shorter dependency.
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

    /// Returns whether all affected declarations are adjacent in the full section.
    fn declarations_are_contiguous(&self) -> bool {
        // Locate affected declarations in the section's complete source order.
        let positions = self.affected.iter().filter_map(|affected| {
            self.section
                .participants
                .iter()
                .position(|participant| participant.def_id == affected.participant.def_id)
        });

        // Require every affected declaration to occupy the next source position.
        let indices = positions.collect::<Vec<_>>();
        indices.windows(2).all(|window| window[1] == window[0] + 1)
    }

    /// Builds coherent replacements from an owner-child dependency relationship.
    fn dependency_renames(
        &self,
        relationship: FamilyCandidateRelationship,
        confidence: &mut ConfidenceEvidence,
    ) -> Option<Vec<FamilyInferenceRename<'section>>> {
        let owner_name = &self.affected[relationship.owner_index].name;
        if owner_name.len() <= 1 {
            return None;
        }
        let stem = &owner_name.words[..owner_name.len() - 1];
        let mut renames = Vec::new();
        for (index, affected) in self.affected.iter().enumerate() {
            // Derive each role from its owner-child position in the dependency family.
            let replacement = if index == relationship.owner_index {
                affected.name.join()
            } else if index == relationship.child_index {
                let mut words = stem.to_vec();
                words.push(affected.name.last()?.to_owned());
                confidence.discard_tokens(affected.name.len() - 1);
                words.concat()
            } else {
                affected.name.join()
            };

            // Associate the inferred concept-first role with its declaration.
            renames.push(FamilyInferenceRename {
                participant: affected.participant,
                replacement,
            });
        }
        Some(renames)
    }

    /// Selects relationship-aware replacements, falling back to prefix removal.
    fn renames(
        &mut self,
        reverse_owner: Option<FamilyCandidateRelationship>,
        dependency_relationship: Option<FamilyCandidateRelationship>,
        confidence: &mut ConfidenceEvidence,
    ) -> Vec<FamilyInferenceRename<'section>> {
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
        } else if let Some(relationship) = dependency_relationship
            && let Some(inferred) = self.dependency_renames(relationship, confidence)
        {
            renames = inferred;
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
        renames
    }

    /// Scores semantic evidence and returns a finding when the inference is actionable.
    fn infer(
        mut self,
        context: Option<&str>,
        occupied_names: &HashSet<String>,
    ) -> Option<FamilyNameFinding> {
        // Require affected declarations and contextual or ownership evidence.
        if self.affected.is_empty() {
            return None;
        }
        let prefix = NameTokens::pascal(&self.section.prefix);
        let context_matches = prefix.len() >= 2
            && context
                .map(NameTokens::context)
                .is_some_and(|context| prefix.matches_context(&context));

        // Combine contextual evidence with any reverse owner-child relationship.
        let reverse_owner = self.reverse_owner_relationship();
        if !context_matches && reverse_owner.is_none() {
            return None;
        }

        // Detect an exact family root and reject already coherent single-item families.
        let root = self
            .section
            .participants
            .iter()
            .find(|participant| participant.name == self.section.prefix);
        let exact_root = root.is_some();
        if self.single_participant_is_semantically_rooted(root) {
            return None;
        }

        // Gather dependency structure and initialize confidence scoring.
        let dependency_relationship = self.dependency_relationship();
        let competing_stems = self.dependency_stems().len() > 1;
        let mut confidence = ConfidenceEvidence::default();
        let mut evidence = Vec::new();

        // Score the strongest contextual and dependency evidence first.
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

        // Add structural confidence from the affected declaration set.
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

        // Account for source locality and ambiguous competing dependency stems.
        if self.declarations_are_contiguous() {
            confidence.add(ConfidenceSignal::Contiguous);
        }
        if competing_stems {
            confidence.add(ConfidenceSignal::CompetingStem);
            evidence.push("multiple dependency owners provide competing family stems".to_owned());
        }

        let renames = self.renames(reverse_owner, dependency_relationship, &mut confidence);

        // Resolve scored evidence and proposed names into an actionable finding.
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
/// Identifier split into normalized `PascalCase` word tokens.
#[derive(Clone, Debug, Eq, PartialEq)]
struct NameTokens {
    /// Ordered semantic words extracted from the identifier.
    words: Vec<String>,
}

impl NameTokens {
    /// Tokenizes a `PascalCase` declaration or section name.
    fn pascal(value: &str) -> Self {
        Self {
            words: identifier_case::words_from_pascal(value),
        }
    }

    /// Tokenizes an enclosing module name in its native identifier case.
    fn context(value: &str) -> Self {
        Self {
            words: identifier_case::words(value),
        }
    }

    /// Returns whether tokenization produced no semantic words.
    const fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    /// Returns the number of semantic words in the name.
    const fn len(&self) -> usize {
        self.words.len()
    }

    /// Returns the final role-bearing word, if present.
    fn last(&self) -> Option<&str> {
        self.words.last().map(String::as_str)
    }

    /// Reconstructs the normalized `PascalCase` identifier.
    fn join(&self) -> String {
        self.words.concat()
    }

    /// Removes an exact leading word sequence from this name.
    fn strip_prefix(&self, prefix: &Self) -> Option<Self> {
        self.words.starts_with(&prefix.words).then(|| Self {
            words: self.words[prefix.len()..].to_vec(),
        })
    }

    /// Compares context words while permitting singular/plural variation at the end.
    fn matches_context(&self, context: &Self) -> bool {
        if self.words.len() != context.words.len() {
            return false;
        }
        let paired = self.words.iter().zip(&context.words);

        // Permit a plural variation only on the final contextual word.
        paired.enumerate().all(|(index, (left, right))| {
            if left == right {
                return true;
            }
            index + 1 == self.words.len()
                && (left.strip_suffix('s').is_some_and(|left| left == right)
                    || right.strip_suffix('s').is_some_and(|right| left == right))
        })
    }
}

// -----------------------------------------------------------------------------
// ModuleNamingAnalysis: Module level semantic context
// -----------------------------------------------------------------------------

/// Module-wide naming context and semantic relationships used by family inference.
struct ModuleNamingAnalysis {
    /// Enclosing inline-module identifier or file stem.
    context: Option<String>,
    /// Local declaration dependencies resolved from HIR.
    dependencies: HashMap<LocalDefId, HashSet<LocalDefId>>,
    /// Authored nominal declarations eligible for family analysis.
    participants: Vec<SectionParticipant>,
    /// Existing nominal names that proposed replacements must not collide with.
    occupied_names: HashSet<String>,
}

impl ModuleNamingAnalysis {
    /// Collects naming context, declarations, dependencies, and occupied names.
    fn collect(cx: &LateContext<'_>, module: &Mod<'_>, hir_id: HirId) -> Self {
        // Resolve direct module declarations before filtering authored nominal types.
        let resolved = module
            .item_ids
            .iter()
            .map(|item_id| cx.tcx.hir_item(*item_id));

        // Retain nominal declarations written directly in the module.
        let authored = resolved
            .filter(|item| !item.span.from_expansion() && !item.is_framework_generated())
            .filter_map(|item| {
                Self::nominal_name(cx, item.kind, item.owner_id.def_id).map(|name| {
                    SectionParticipant {
                        def_id: item.owner_id.def_id,
                        name,
                        span: item.span,
                        is_nominal: true,
                    }
                })
            });

        // Materialize participants before deriving occupied namespace names.
        let participants = authored.collect::<Vec<_>>();

        // Reserve every existing nominal name before proposing concise replacements.
        let occupied_names = participants
            .iter()
            .map(|participant| participant.name.clone())
            .collect();

        // Combine naming context, dependency evidence, and namespace occupancy.
        Self {
            context: Self::context(cx, module, hir_id),
            dependencies: Self::dependencies(cx, module),
            participants,
            occupied_names,
        }
    }

    /// Resolves the inline-module identifier or file stem that supplies context words.
    fn context(cx: &LateContext<'_>, module: &Mod<'_>, hir_id: HirId) -> Option<String> {
        for node in [cx.tcx.hir_node(hir_id), cx.tcx.parent_hir_node(hir_id)] {
            let Node::Item(item) = node else { continue };
            let Some(identifier) = item.kind.ident() else {
                continue;
            };
            return Some(identifier.name.to_string());
        }
        let source_map = cx.sess().source_map();
        let filename = source_map.span_to_filename(module.spans.inner_span);
        filename.into_local_path().and_then(|path| {
            path.file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
        })
    }

    /// Resolves a direct inherent implementation to its local nominal self type.
    fn impl_dependency_owner(cx: &LateContext<'_>, item: &Item<'_>) -> Option<LocalDefId> {
        let self_type = cx.tcx.type_of(item.owner_id).instantiate_identity();
        let ty::Adt(definition, _) = self_type.kind() else {
            return None;
        };
        definition.did().as_local()
    }

    /// Collects definition dependencies for nominal declarations and their direct impls.
    fn dependencies(
        cx: &LateContext<'_>,
        module: &Mod<'_>,
    ) -> HashMap<LocalDefId, HashSet<LocalDefId>> {
        let mut dependencies = HashMap::<LocalDefId, HashSet<LocalDefId>>::new();
        for item_id in module.item_ids {
            // Resolve one direct item and merge nominal declaration dependencies.
            let item = cx.tcx.hir_item(*item_id);
            if Self::nominal_name(cx, item.kind, item.owner_id.def_id).is_some() {
                dependencies
                    .entry(item.owner_id.def_id)
                    .or_default()
                    .extend(item.dependencies(cx.tcx));
                continue;
            }

            // Merge direct implementation dependencies under their nominal owner.
            let ItemKind::Impl(_) = item.kind else {
                continue;
            };
            let Some(definition) = Self::impl_dependency_owner(cx, item) else {
                continue;
            };

            // Merge implementation references under the resolved nominal owner.
            dependencies
                .entry(definition)
                .or_default()
                .extend(item.dependencies(cx.tcx));
        }
        dependencies
    }

    /// Returns the name of a nominal declaration kind, excluding value-level items.
    fn nominal_name(
        cx: &LateContext<'_>,
        kind: ItemKind<'_>,
        def_id: LocalDefId,
    ) -> Option<String> {
        // Classify concrete nominal declarations.
        let is_concrete = matches!(
            kind,
            ItemKind::Struct(..) | ItemKind::Enum(..) | ItemKind::Union(..)
        );

        // Classify abstract nominal declarations before resolving their name.
        let is_abstract = matches!(
            kind,
            ItemKind::TyAlias(..) | ItemKind::Trait(..) | ItemKind::TraitAlias(..)
        );
        (is_concrete || is_abstract).then(|| cx.tcx.item_name(def_id.to_def_id()).to_string())
    }
}

// -----------------------------------------------------------------------------
// Tests: Unit tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::{
        ConfidenceEvidence, ConfidenceNameAvailability, ConfidenceSignal, NameTokens,
        THRESHOLD_REPORT, THRESHOLD_SUGGESTION,
    };
    use crate::utils::identifier_case;

    #[test]
    fn tokenizes_pascal_case_and_acronyms() {
        assert_eq!(
            identifier_case::words_from_pascal("HttpServerConfig"),
            ["Http", "Server", "Config"]
        );
        assert_eq!(
            identifier_case::words_from_pascal("MigrationEdits"),
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
        assert!(
            !NameTokens::pascal("SectionsDivider")
                .matches_context(&NameTokens::context("section_divider"))
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
            ConfidenceNameAvailability::Available
        ));
        assert!(!ConfidenceEvidence::allows_exact_names(
            THRESHOLD_SUGGESTION,
            ConfidenceNameAvailability::Occupied
        ));
        assert!(!ConfidenceEvidence::allows_exact_names(
            THRESHOLD_SUGGESTION - 1,
            ConfidenceNameAvailability::Available
        ));
    }
}
