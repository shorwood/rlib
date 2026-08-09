extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_span;

use std::collections::HashMap;

use rustc_hir::{HirId, Item, ItemKind, Mod, Node};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty;
use rustc_span::{BytePos, Span};

use super::config::LibraryConfig;
use super::{identifier_case, prose_case};

// -----------------------------------------------------------------------------
// Section: Shared organization analysis data
// -----------------------------------------------------------------------------

/// One source-level problem found by the shared analyzer.
pub struct SectionFinding {
    /// Authored source range to underline or replace.
    pub(crate) span: Span,
    /// Primary explanation of the organization problem.
    pub(crate) message: String,
    /// Naming- or organization-first remediation guidance.
    pub(crate) help: String,
    /// Canonical divider text when a safe replacement can be synthesized.
    pub(crate) replacement: Option<String>,
}

/// One distinct declaration covered by a valid section.
pub struct SectionParticipant {
    /// Local definition represented by this declaration or implementation.
    pub(crate) def_id: rustc_hir::def_id::LocalDefId,
    /// Authored declaration name used for family inference and diagnostics.
    pub(crate) name: String,
    /// Source range occupied by the declaration.
    pub(crate) span: Span,
    /// Whether this participant defines a nominal type rather than supporting it.
    pub(crate) is_nominal: bool,
}

/// One valid authored section available to semantic companion lints.
pub struct SectionGroup {
    /// One-based position of the section in its source module.
    pub(crate) ordinal: usize,
    /// `PascalCase` family prefix declared by the divider.
    pub(crate) prefix: String,
    /// Complete source range of the divider template.
    pub(crate) span: Span,
    /// Distinct declarations governed by the divider.
    pub(crate) participants: Vec<SectionParticipant>,
}

/// Findings split by lint identity so each rule remains independently configurable.
#[derive(Default)]
pub struct SectionAnalysis {
    /// Declaration groups that have no preceding divider.
    pub(crate) missing: Vec<SectionFinding>,
    /// Dividers whose syntax, width, or placement is invalid.
    pub(crate) malformed: Vec<SectionFinding>,
    /// Repeated section family prefixes within one module.
    pub(crate) duplicates: Vec<SectionFinding>,
    /// Dividers whose prefix disagrees with the declarations they contain.
    pub(crate) mismatches: Vec<SectionFinding>,
    /// Dividers governing more distinct declarations than the configured limit.
    pub(crate) overloaded: Vec<SectionFinding>,
    /// Valid sections available to companion semantic lints.
    pub(crate) sections: Vec<SectionGroup>,
}

impl SectionAnalysis {
    /// Returns the authored section containing an item at the given source span.
    pub(crate) fn section_ordinal_for_span(&self, span: Span) -> Option<usize> {
        let preceding_section = self
            .sections
            .iter()
            .rev()
            .find(|section| section.span.lo() <= span.lo());
        preceding_section.map(|section| section.ordinal)
    }
}

// -----------------------------------------------------------------------------
// SectionAnalyzer: Source analysis
// -----------------------------------------------------------------------------

/// Authored content and indentation used to render one divider replacement.
#[derive(Clone, Copy)]
struct SectionAnalyzerRenderRequest<'source> {
    /// Canonical placeholder content.
    content: &'source str,
    /// Whitespace preceding the divider in its module.
    indentation: &'source str,
}

/// Parses configured divider blocks and relates them to module-level declaration groups.
pub struct SectionAnalyzer {
    /// Parsed divider template used for both recognition and rendering.
    template: Template,
    /// Maximum permitted width of every rendered divider line.
    max_line_length: usize,
    /// Maximum number of distinct declarations governed by one divider.
    max_declarations_per_section: usize,
}

impl SectionAnalyzer {
    /// Loads and validates the shared library configuration.
    pub(crate) fn from_config() -> Self {
        // Load the divider-specific project configuration.
        let config = LibraryConfig::load().section_dividers;
        config
            .validate()
            .unwrap_or_else(|message| panic!("invalid section divider configuration: {message}"));

        // Parse the source template before retaining the complete semantic policy.
        // Fail during lint construction when the project template is invalid.
        let parsed_template = Template::parse(&config.template, config.max_line_length);
        let template = parsed_template
            .unwrap_or_else(|message| panic!("invalid section divider template: {message}"));

        // Retain the parsed template beside both configured limits.
        Self {
            template,
            max_line_length: config.max_line_length,
            max_declarations_per_section: config.max_declarations_per_section,
        }
    }

    /// Converts a snippet-relative template match into an absolute source event.
    fn divider_from_match(span: Span, matched: TemplateMatch) -> SectionEventDivider {
        // Convert snippet offsets into source-map byte positions.
        let start = u32::try_from(matched.start).expect("source span offset should fit in BytePos");
        let end = u32::try_from(matched.end).expect("source span offset should fit in BytePos");

        // Preserve the template's exact authored extent and indentation.
        let divider_span = span
            .with_lo(span.lo() + BytePos(start))
            .with_hi(span.lo() + BytePos(end));

        // Carry authored content forward for semantic validation and fixes.
        SectionEventDivider {
            span: divider_span,
            raw_content: matched.content,
            indentation: matched.indentation,
        }
    }

    /// Analyzes one source module without inspecting comments inside item bodies.
    pub(crate) fn analyze(
        &self,
        cx: &LateContext<'_>,
        module: &Mod<'_>,
        hir_id: HirId,
    ) -> SectionAnalysis {
        ModuleAnalysis::analyze(self, cx, module, hir_id)
    }

    /// Finds divider templates in an item-free source range.
    fn dividers_in_span(&self, cx: &LateContext<'_>, span: Span) -> Vec<SectionEventDivider> {
        let Ok(source) = cx.sess().source_map().span_to_snippet(span) else {
            return Vec::new();
        };

        // Translate template-relative offsets back into source-map spans.
        let matches = self.template.find_matches(&source);
        matches
            .into_iter()
            .map(|matched| Self::divider_from_match(span, matched))
            .collect()
    }

    /// Returns whether rendering `content` stays within the configured line width.
    fn rendered_lines_fit(&self, content: &str) -> bool {
        self.template
            .render(content)
            .lines()
            .all(|line| line.chars().count() <= self.max_line_length)
    }

    /// Renders an indented canonical divider when it satisfies the width policy.
    fn render_replacement(&self, request: SectionAnalyzerRenderRequest<'_>) -> Option<String> {
        self.rendered_lines_fit(request.content).then(|| {
            let rendered = self.template.render(request.content);
            rendered
                .lines()
                .collect::<Vec<_>>()
                .join(&format!("\n{}", request.indentation))
        })
    }
}

// -----------------------------------------------------------------------------
// Module: Complete module analysis
// -----------------------------------------------------------------------------

/// Authored and inferred names involved in one section-prefix mismatch.
#[derive(Clone, Copy)]
struct ModulePrefixMismatch<'name> {
    /// Prefix written in the divider.
    authored: &'name str,
    /// Shared prefix inferred from the declarations.
    expected: &'name str,
}

/// Validated section prefix passed through module-level checks.
#[derive(Clone, Copy)]
struct ModuleSectionPrefix<'name> {
    /// Authored `PascalCase` prefix.
    text: &'name str,
}

/// Semantic namespace supplied by a named containing module.
struct ModuleNamespace {
    /// Canonical `PascalCase` form of the module name.
    prefix: String,
}

/// Builds and validates the ordered section event stream for one source module.
struct ModuleAnalysis;

impl ModuleAnalysis {
    /// Appends every divider found in `span` to the module event stream.
    fn append_dividers(
        analyzer: &SectionAnalyzer,
        cx: &LateContext<'_>,
        span: Span,
        events: &mut Vec<SectionEventStreamEntry>,
    ) {
        let dividers = analyzer.dividers_in_span(cx, span);
        events.extend(dividers.into_iter().map(SectionEventStreamEntry::Divider));
    }

    /// Collects authored items and dividers, then analyzes them in source order.
    fn analyze(
        analyzer: &SectionAnalyzer,
        cx: &LateContext<'_>,
        module: &Mod<'_>,
        hir_id: HirId,
    ) -> SectionAnalysis {
        // Identify the physical source file represented by this HIR module.
        let source_map = cx.sess().source_map();
        let module_span = Self::source_span(cx, module, hir_id);
        let module_file = source_map.span_to_filename(module_span);

        // Retain only authored declarations from the module's physical file.
        let source_items = module
            .item_ids
            .iter()
            .map(|item_id| cx.tcx.hir_item(*item_id));

        // Ignore expansions and declarations sourced from child module files.
        let mut items = source_items
            .filter(|item| {
                !item.span.from_expansion() && source_map.span_to_filename(item.span) == module_file
            })
            .collect::<Vec<_>>();

        // Establish source order before constructing the section event stream.
        items.sort_unstable_by_key(|item| item.span.lo());

        // Interleave authored dividers with the declarations they introduce.
        let mut events = Vec::new();
        let mut previous = module_span.lo();
        for item in items {
            if previous <= item.span.lo() {
                let gap = module_span.with_lo(previous).with_hi(item.span.lo());
                Self::append_dividers(analyzer, cx, gap, &mut events);
            }
            if let Some(participant) = SectionEventCandidate::from_item(cx, item) {
                events.push(SectionEventStreamEntry::Candidate(participant));
            }
            previous = previous.max(item.span.hi());
        }
        if previous <= module_span.hi() {
            let gap = module_span.with_lo(previous).with_hi(module_span.hi());
            Self::append_dividers(analyzer, cx, gap, &mut events);
        }
        events.sort_unstable_by_key(SectionEventStreamEntry::position);
        let namespace = Self::namespace(cx, hir_id);
        Self::analyze_events(analyzer, events, namespace)
    }

    /// Reduces a complete event stream into independently reportable findings.
    fn analyze_events(
        analyzer: &SectionAnalyzer,
        events: Vec<SectionEventStreamEntry>,
        namespace: Option<ModuleNamespace>,
    ) -> SectionAnalysis {
        let mut state = SectionEventStreamState {
            namespace,
            ..SectionEventStreamState::default()
        };

        // Close each group when the following divider starts a new one.
        for event in events {
            state.apply(analyzer, event);
        }
        state.finish(analyzer)
    }

    /// Resolves the semantic namespace of a named containing module.
    fn namespace(cx: &LateContext<'_>, hir_id: HirId) -> Option<ModuleNamespace> {
        let containing_nodes = [cx.tcx.hir_node(hir_id), cx.tcx.parent_hir_node(hir_id)];
        let item = containing_nodes.into_iter().find_map(|node| match node {
            Node::Item(item) if matches!(item.kind, ItemKind::Mod(..)) => Some(item),
            _ => None,
        })?;
        let ident = item.kind.ident()?;
        Some(ModuleNamespace {
            prefix: identifier_case::to_pascal(ident.name.as_str()),
        })
    }

    /// Finds the physical source extent in which module-level dividers may appear.
    fn source_span(cx: &LateContext<'_>, module: &Mod<'_>, hir_id: HirId) -> Span {
        // Resolve the inline module item surrounding the HIR module body.
        let inner = module.spans.inner_span;
        let containing_nodes = [cx.tcx.hir_node(hir_id), cx.tcx.parent_hir_node(hir_id)];
        let item = containing_nodes.into_iter().find_map(|node| match node {
            Node::Item(item) if matches!(item.kind, ItemKind::Mod(..)) => Some(item),
            _ => None,
        });

        // File modules already expose the complete inner span from HIR.
        let Some(item) = item else {
            return inner;
        };
        if cx.sess().source_map().span_to_filename(item.span)
            != cx.sess().source_map().span_to_filename(inner)
        {
            return inner;
        }

        // Expand inline-module bodies to include comments beside their braces.
        let source_map = cx.sess().source_map();
        let opening_source = source_map.span_to_snippet(item.span.with_hi(inner.lo()));
        let opening_brace = opening_source.ok().and_then(|source| source.rfind('{'));
        let opening_offset = opening_brace
            .and_then(|offset| u32::try_from(offset + 1).ok())
            .map(BytePos);
        let lo = opening_offset.map_or_else(|| inner.lo(), |offset| item.span.lo() + offset);

        // Extend through the authored closing brace when its source is available.
        let closing_source = source_map.span_to_snippet(item.span.with_lo(inner.hi()));
        let closing_brace = closing_source.ok().and_then(|source| source.find('}'));
        let closing_offset = closing_brace
            .and_then(|offset| u32::try_from(offset).ok())
            .map(BytePos);
        let hi = closing_offset.map_or_else(|| inner.hi(), |offset| inner.hi() + offset);
        inner.with_lo(lo).with_hi(hi)
    }

    /// Records placement, syntax, and width failures for one authored divider.
    fn record_malformed_section(
        analyzer: &SectionAnalyzer,
        section: &SectionEventStreamGroup,
        parsed: &ParsedContent,
        analysis: &mut SectionAnalysis,
    ) {
        if section.participants.is_empty() {
            analysis.malformed.push(SectionFinding {
                span: section.divider.span,
                message: "section divider does not contain any module declarations".to_owned(),
                help: "remove this divider or place it before the declarations it describes"
                    .to_owned(),
                replacement: None,
            });
        }

        // Prefer a safe canonical replacement for syntactic content errors.
        if let Some(message) = &parsed.error {
            // Render the normalized content only when the template can represent it safely.
            let replacement = parsed.normalized.as_ref().and_then(|content| {
                analyzer.render_replacement(SectionAnalyzerRenderRequest {
                    content,
                    indentation: &section.divider.indentation,
                })
            });

            // Explain the canonical grammar alongside any machine-applicable fix.
            analysis.malformed.push(SectionFinding {
                span: section.divider.span,
                message: message.clone(),
                help: "use `PascalCasePrefix` or `PascalCasePrefix: Sentence case description`"
                    .to_owned(),
                replacement,
            });
        } else if !analyzer.rendered_lines_fit(&section.divider.raw_content) {
            // Describe the configured width failure independently from syntax errors.
            let message = format!(
                "section divider exceeds the configured {}-character line limit",
                analyzer.max_line_length
            );
            let help =
                "shorten the optional description or choose a more compact template".to_owned();

            // Record the width failure without proposing an unsafe content rewrite.
            analysis.malformed.push(SectionFinding {
                span: section.divider.span,
                message,
                help,
                replacement: None,
            });
        }
    }

    /// Publishes a syntactically valid, nonempty section for companion analyses.
    fn record_valid_section(
        analyzer: &SectionAnalyzer,
        section: &SectionEventStreamGroup,
        parsed: &ParsedContent,
        prefix: ModuleSectionPrefix<'_>,
        analysis: &mut SectionAnalysis,
    ) {
        // Require canonical syntax, width, and at least one declaration participant.
        let is_valid = parsed.error.is_none()
            && analyzer.rendered_lines_fit(&section.divider.raw_content)
            && !section.participants.is_empty();
        if !is_valid {
            return;
        }

        // Collapse a nominal declaration and all of its implementation blocks into one concept.
        let participants = section.participants.distinct_declarations();
        if participants.len() > analyzer.max_declarations_per_section {
            // Explain the measurable excess at the divider that owns the broad family.
            let message = format!(
                "section `{}` contains {} distinct declarations, exceeding the configured maximum of {}",
                prefix.text,
                participants.len(),
                analyzer.max_declarations_per_section
            );

            // Steer remediation toward concepts and names before mechanical splitting.
            let help = "reconsider whether these declarations express smaller coherent concepts and rename them around those concepts; add another divider only for a genuinely independent family".to_owned();
            analysis.overloaded.push(SectionFinding {
                span: section.divider.span,
                message,
                help,
                replacement: None,
            });
        }

        // Publish the stable semantic section used by companion naming analyses.
        analysis.sections.push(SectionGroup {
            ordinal: analysis.sections.len() + 1,
            prefix: prefix.text.to_owned(),
            span: section.divider.span,
            participants,
        });
    }

    /// Records a finding when `prefix` has already appeared in the module.
    fn record_duplicate_section(
        section: &SectionEventStreamGroup,
        prefix: ModuleSectionPrefix<'_>,
        seen_prefixes: &mut HashMap<String, Span>,
        analysis: &mut SectionAnalysis,
    ) {
        // Register this prefix and stop when it has not appeared before.
        if seen_prefixes
            .insert(prefix.text.to_owned(), section.divider.span)
            .is_none()
        {
            return;
        }

        // Report the later occurrence while naming the family it should rejoin.
        let message = format!(
            "section prefix `{}` is used more than once in this module",
            prefix.text
        );
        let help = format!(
            "combine this family with the earlier `{}` section",
            prefix.text
        );

        // Attach the prepared diagnostic text to the later divider occurrence.
        analysis.duplicates.push(SectionFinding {
            span: section.divider.span,
            message,
            help,
            replacement: None,
        });
    }

    /// Builds a finding for a valid prefix that differs from the inferred family name.
    fn wrong_prefix_finding(
        section: &SectionEventStreamGroup,
        mismatch: ModulePrefixMismatch<'_>,
    ) -> SectionFinding {
        // Explain the mismatch and the inferred declaration-family prefix.
        let message = format!(
            "section prefix `{}` does not match its declaration family",
            mismatch.authored
        );
        let help = format!(
            "rename the declarations into one coherent family first; their longest shared PascalCase prefix is `{}`, and a new section is appropriate only for an independent concept",
            mismatch.expected
        );

        // Attach the naming-first guidance to the authored divider.
        SectionFinding {
            span: section.divider.span,
            message,
            help,
            replacement: None,
        }
    }

    /// Builds naming-first guidance for declarations that share no `PascalCase` prefix.
    fn unrelated_names_finding(
        section: &SectionEventStreamGroup,
        prefix: ModuleSectionPrefix<'_>,
    ) -> SectionFinding {
        // Explain both the absent shared prefix and the declarations that need renaming.
        let message = format!(
            "section `{}` contains declarations without a shared PascalCase prefix",
            prefix.text
        );
        let help = format!(
            "reconsider the names {} so closely related declarations share a visible prefix; split the section only when they represent independent concepts",
            section.participants.formatted_names()
        );

        // Attach naming-first guidance to the authored divider.
        SectionFinding {
            span: section.divider.span,
            message,
            help,
            replacement: None,
        }
    }

    /// Compares an authored prefix with the names grouped beneath it.
    fn mismatch_finding(
        section: &SectionEventStreamGroup,
        prefix: ModuleSectionPrefix<'_>,
        namespace: Option<&ModuleNamespace>,
    ) -> Option<SectionFinding> {
        // Infer a family only when the divider actually governs declarations.
        if section.participants.is_empty() {
            return None;
        }
        if namespace.is_some_and(|namespace| namespace.prefix == prefix.text) {
            return None;
        }
        let names = section.participants.names();

        // Report declarations with no shared naming root directly.
        let Some(expected) = identifier_case::longest_common_pascal_prefix(&names) else {
            return Some(Self::unrelated_names_finding(section, prefix));
        };
        if expected == prefix.text {
            return None;
        }

        // Report a concrete mismatch using the inferred family prefix.
        Some(Self::wrong_prefix_finding(
            section,
            ModulePrefixMismatch {
                authored: prefix.text,
                expected: &expected,
            },
        ))
    }

    /// Runs every section-level check when a divider group closes.
    fn finish_section(
        analyzer: &SectionAnalyzer,
        section: &SectionEventStreamGroup,
        namespace: Option<&ModuleNamespace>,
        seen_prefixes: &mut HashMap<String, Span>,
        analysis: &mut SectionAnalysis,
    ) {
        // Parse the divider content to validate its syntax and extract the authored prefix.
        let parsed = ParsedContent::from(section.divider.raw_content.as_str());
        Self::record_malformed_section(analyzer, section, &parsed, analysis);
        let Some(prefix) = parsed.prefix.as_deref() else {
            return;
        };
        let prefix = ModuleSectionPrefix { text: prefix };

        // Record the section independently for each semantic companion lint.
        Self::record_valid_section(analyzer, section, &parsed, prefix, analysis);
        Self::record_duplicate_section(section, prefix, seen_prefixes, analysis);
        let Some(finding) = Self::mismatch_finding(section, prefix, namespace) else {
            return;
        };
        analysis.mismatches.push(finding);
    }
}

// -----------------------------------------------------------------------------
// SectionEvent: Section events and participants
// -----------------------------------------------------------------------------

/// Authored divider data carried through section event analysis.
struct SectionEventDivider {
    /// Complete source range occupied by the divider template.
    span: Span,
    /// Unparsed content captured from the template placeholder.
    raw_content: String,
    /// Whitespace that prefixes each rendered divider line.
    indentation: String,
}

/// Semantic role of a named declaration within a source section.
#[derive(Clone, Copy)]
enum SectionEventCandidateKind {
    /// A type-like declaration that establishes a nominal concept.
    Nominal,
    /// A value declaration that supports a surrounding concept.
    Supporting,
}

/// Module declaration that may participate in the current section.
struct SectionEventCandidate {
    /// Local definition represented by the declaration.
    def_id: rustc_hir::def_id::LocalDefId,
    /// Authored declaration name used for family inference.
    name: String,
    /// Complete declaration source range.
    span: Span,
    /// Whether this candidate introduces a nominal type.
    is_nominal_declaration: bool,
}

impl SectionEventCandidate {
    /// Builds a candidate from an item whose definition and name are direct.
    fn from_named_item(
        cx: &LateContext<'_>,
        item: &Item<'_>,
        kind: SectionEventCandidateKind,
    ) -> Self {
        Self {
            def_id: item.owner_id.def_id,
            name: cx.tcx.item_name(item.owner_id.to_def_id()).to_string(),
            span: item.span,
            is_nominal_declaration: matches!(kind, SectionEventCandidateKind::Nominal),
        }
    }

    /// Converts a direct inherent implementation into a candidate for its self type.
    fn from_impl(cx: &LateContext<'_>, item: &Item<'_>) -> Option<Self> {
        // Resolve the implementation's nominal self type and local definition.
        let self_type = cx.tcx.type_of(item.owner_id).instantiate_identity();
        let ty::Adt(definition, _) = self_type.kind() else {
            return None;
        };
        let definition = definition.did().as_local()?;

        // Represent the supporting implementation under its nominal type's family name.
        Some(Self {
            def_id: definition,
            name: cx.tcx.item_name(definition.to_def_id()).to_string(),
            span: item.span,
            is_nominal_declaration: false,
        })
    }

    /// Converts a section-relevant module item into a source candidate.
    fn from_item(cx: &LateContext<'_>, item: &Item<'_>) -> Option<Self> {
        // Ignore declarations whose source was synthesized by expansion.
        if item.span.from_expansion() {
            return None;
        }

        // Classify concrete nominal declarations directly.
        if matches!(
            item.kind,
            ItemKind::Struct(..) | ItemKind::Enum(..) | ItemKind::Union(..)
        ) {
            return Some(Self::from_named_item(
                cx,
                item,
                SectionEventCandidateKind::Nominal,
            ));
        }

        // Classify abstract nominal declarations directly.
        if matches!(
            item.kind,
            ItemKind::TyAlias(..) | ItemKind::Trait(..) | ItemKind::TraitAlias(..)
        ) {
            return Some(Self::from_named_item(
                cx,
                item,
                SectionEventCandidateKind::Nominal,
            ));
        }

        // Classify value declarations and supporting inherent implementations.
        match item.kind {
            ItemKind::Const(..) | ItemKind::Static(..) | ItemKind::Fn { .. } => Some(
                Self::from_named_item(cx, item, SectionEventCandidateKind::Supporting),
            ),
            ItemKind::Impl(_) => Self::from_impl(cx, item),
            _ => None,
        }
    }
}

// -----------------------------------------------------------------------------
// SectionEventStream: Ordered section reduction
// -----------------------------------------------------------------------------

/// One divider or declaration in the module's source-ordered event stream.
enum SectionEventStreamEntry {
    /// A recognized divider template starts a new section.
    Divider(
        /// Divider source and captured content that open the section.
        SectionEventDivider,
    ),
    /// A declaration belongs to the nearest preceding divider, if any.
    Candidate(
        /// Authored declaration associated with the active section.
        SectionEventCandidate,
    ),
}

impl SectionEventStreamEntry {
    /// Returns the event's source position for stable ordering.
    fn position(&self) -> BytePos {
        match self {
            Self::Divider(divider) => divider.span.lo(),
            Self::Candidate(participant) => participant.span.lo(),
        }
    }
}

#[derive(Default)]
/// Declarations accumulated before the next divider boundary.
struct SectionEventStreamCandidates(
    /// Authored candidates retained in source order.
    Vec<SectionEventCandidate>,
);

impl SectionEventStreamCandidates {
    /// Collapses a declaration and its impls into one nominal participant.
    fn record_distinct_declaration(
        positions: &mut HashMap<rustc_hir::def_id::LocalDefId, usize>,
        declarations: &mut Vec<SectionParticipant>,
        candidate: SectionParticipant,
    ) {
        let Some(index) = positions.get(&candidate.def_id).copied() else {
            positions.insert(candidate.def_id, declarations.len());
            declarations.push(candidate);
            return;
        };
        if !candidate.is_nominal || declarations[index].is_nominal {
            return;
        }
        declarations[index] = candidate;
    }

    /// Removes all accumulated declarations after their group has been handled.
    fn clear(&mut self) {
        self.0.clear();
    }

    /// Returns whether the group contains no declarations.
    const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Appends one declaration in source order.
    fn push(&mut self, participant: SectionEventCandidate) {
        self.0.push(participant);
    }

    /// Returns whether the accumulated declarations require an authored section.
    fn requires_divider(&self) -> bool {
        self.0.len() > 1
            || self
                .0
                .iter()
                .any(|participant| participant.is_nominal_declaration)
    }

    /// Returns candidate names in their authored order.
    fn names(&self) -> Vec<&str> {
        self.0
            .iter()
            .map(|participant| participant.name.as_str())
            .collect()
    }

    /// Formats a stable, deduplicated set of candidate names for diagnostics.
    fn formatted_names(&self) -> String {
        // Stabilize and deduplicate authored participant names.
        let mut names = self.names();
        names.sort_unstable();
        names.dedup();

        // Quote and join the stable set for diagnostic prose.
        let quoted_names = names
            .into_iter()
            .map(|name| format!("`{name}`"))
            .collect::<Vec<_>>();
        quoted_names.join(", ")
    }

    /// Collapses supporting impls and returns distinct declarations in source order.
    fn distinct_declarations(&self) -> Vec<SectionParticipant> {
        let mut positions = HashMap::<rustc_hir::def_id::LocalDefId, usize>::new();
        let mut declarations = Vec::<SectionParticipant>::new();
        for participant in &self.0 {
            let candidate = SectionParticipant {
                def_id: participant.def_id,
                name: participant.name.clone(),
                span: participant.span,
                is_nominal: participant.is_nominal_declaration,
            };
            Self::record_distinct_declaration(&mut positions, &mut declarations, candidate);
        }

        // Return declarations in authored order after collapsing their impls.
        declarations.sort_unstable_by_key(|participant| participant.span.lo());
        declarations
    }

    /// Produces naming-first guidance for a declaration group without a divider.
    fn missing_guidance(&self) -> String {
        let names = self.names();

        // Turn the inferred prefix, or its absence, into naming-first guidance.
        identifier_case::longest_common_pascal_prefix(&names).map_or_else(
            || format!(
                "reconsider the names {} so related declarations share a visible prefix, then add a divider; create separate sections only for independent concepts",
                self.formatted_names()
            ),
            |prefix| format!(
                "add a divider for `{prefix}`, after first checking whether any outlier should be renamed into that family"
            ),
        )
    }

    /// Builds the missing-divider finding for this nonempty declaration group.
    fn missing_finding(&self) -> SectionFinding {
        // Prefer an inferred family prefix while keeping naming guidance actionable.
        let guidance = self.missing_guidance();
        SectionFinding {
            span: self.0[0].span,
            message: "module declarations are not covered by a section divider".to_owned(),
            help: guidance,
            replacement: None,
        }
    }
}

/// Divider and declarations accumulated beneath it.
struct SectionEventStreamGroup {
    /// Divider that opened the section.
    divider: SectionEventDivider,
    /// Declarations governed by the divider.
    participants: SectionEventStreamCandidates,
}

#[derive(Default)]
/// Mutable reducer state for a module's section event stream.
struct SectionEventStreamState {
    /// Findings and valid sections accumulated so far.
    analysis: SectionAnalysis,
    /// Declarations encountered before any active divider.
    uncovered: SectionEventStreamCandidates,
    /// Divider group currently accepting declarations.
    current: Option<SectionEventStreamGroup>,
    /// First source span associated with each previously used prefix.
    seen_prefixes: HashMap<String, Span>,
    /// Semantic namespace inherited from the containing module.
    namespace: Option<ModuleNamespace>,
}

impl SectionEventStreamState {
    /// Routes a declaration into the active section or the uncovered group.
    fn record_candidate(&mut self, participant: SectionEventCandidate) {
        let Some(section) = &mut self.current else {
            self.uncovered.push(participant);
            return;
        };
        section.participants.push(participant);
    }

    /// Reports and clears declarations accumulated outside a section.
    fn record_uncovered(&mut self) {
        if self.uncovered.requires_divider() {
            self.analysis.missing.push(self.uncovered.missing_finding());
        }
        self.uncovered.clear();
    }

    /// Closes preceding groups and starts the section introduced by `divider`.
    fn start_section(&mut self, analyzer: &SectionAnalyzer, divider: SectionEventDivider) {
        self.record_uncovered();

        // Finish the preceding section before installing its successor.
        if let Some(section) = self.current.take() {
            ModuleAnalysis::finish_section(
                analyzer,
                &section,
                self.namespace.as_ref(),
                &mut self.seen_prefixes,
                &mut self.analysis,
            );
        }
        self.current = Some(SectionEventStreamGroup {
            divider,
            participants: SectionEventStreamCandidates::default(),
        });
    }

    /// Applies one source-ordered event to the reducer state.
    fn apply(&mut self, analyzer: &SectionAnalyzer, event: SectionEventStreamEntry) {
        match event {
            SectionEventStreamEntry::Candidate(participant) => self.record_candidate(participant),
            SectionEventStreamEntry::Divider(divider) => self.start_section(analyzer, divider),
        }
    }

    /// Closes the final groups and returns the completed analysis.
    fn finish(mut self, analyzer: &SectionAnalyzer) -> SectionAnalysis {
        // Preserve declarations that appeared outside any authored section.
        self.record_uncovered();
        let Some(section) = self.current.take() else {
            return self.analysis;
        };

        // Close the final authored section after the event stream ends.
        ModuleAnalysis::finish_section(
            analyzer,
            &section,
            self.namespace.as_ref(),
            &mut self.seen_prefixes,
            &mut self.analysis,
        );

        // Return every finding and validated section accumulated by the reducer.
        self.analysis
    }
}

// -----------------------------------------------------------------------------
// ParsedContent: Parsed divider content
// -----------------------------------------------------------------------------

/// Canonical interpretation of the text captured by a divider template.
struct ParsedContent {
    /// Valid `PascalCase` prefix, if the authored prefix can be used semantically.
    prefix: Option<String>,
    /// Canonical content suitable for a safe replacement.
    normalized: Option<String>,
    /// First syntax or casing failure found in the authored content.
    error: Option<String>,
}

impl From<&str> for ParsedContent {
    /// Parses divider content and computes its canonical representation.
    fn from(content: &str) -> Self {
        // Split and trim the authored prefix and optional description.
        let trimmed = content.trim();
        let (raw_prefix, raw_description) = trimmed
            .split_once(':')
            .map_or((trimmed, None), |(prefix, description)| {
                (prefix.trim(), Some(description.trim()))
            });

        // Normalize both halves before classifying the first canonical-form violation.
        let prefix = identifier_case::is_pascal(raw_prefix).then(|| raw_prefix.to_owned());
        let normalized_description = raw_description.map(prose_case::sentence);
        let normalized =
            Self::normalized_content(prefix.as_deref(), normalized_description.as_deref());

        // Compare the authored content with its canonical representation.
        let error = Self::content_error(
            content,
            prefix.as_ref(),
            raw_description,
            normalized_description.as_deref(),
            normalized.as_deref(),
        );

        // Retain both semantic content and the safest canonical repair.
        Self {
            prefix,
            normalized,
            error,
        }
    }
}

impl ParsedContent {
    /// Joins a valid prefix and optional description using canonical spacing.
    fn normalized_content(prefix: Option<&str>, description: Option<&str>) -> Option<String> {
        match (prefix, description) {
            (Some(prefix), Some(description)) if !description.is_empty() => {
                Some(format!("{prefix}: {description}"))
            }
            (Some(prefix), None) => Some(prefix.to_owned()),
            _ => None,
        }
    }

    /// Returns the first reason authored content differs from its canonical form.
    fn content_error(
        content: &str,
        prefix: Option<&String>,
        raw_description: Option<&str>,
        normalized_description: Option<&str>,
        normalized: Option<&str>,
    ) -> Option<String> {
        if prefix.is_none() {
            Some("section divider prefix is not PascalCase".to_owned())
        } else if raw_description.is_some_and(str::is_empty) {
            Some("section divider description is empty".to_owned())
        } else if raw_description != normalized_description {
            Some("section divider description must use sentence case".to_owned())
        } else if normalized != Some(content) {
            Some("section divider content has non-canonical spacing".to_owned())
        } else {
            None
        }
    }
}

// -----------------------------------------------------------------------------
// Template: Configured template matching and rendering
// -----------------------------------------------------------------------------

/// Static fragments surrounding an optional placeholder on one template line.
struct TemplateLine {
    /// Text that must appear before the captured content.
    before: String,
    /// Text after the placeholder, or `None` for a fully static line.
    after: Option<String>,
}

/// One complete divider template recognized in a source snippet.
struct TemplateMatch {
    /// Byte offset at which non-indentation template text begins.
    start: usize,
    /// Byte offset immediately after the final template line.
    end: usize,
    /// Text captured from the template placeholder.
    content: String,
    /// Whitespace shared by every matched template line.
    indentation: String,
}

/// Source line split into its byte position, indentation, and content.
struct TemplateSourceLine<'source> {
    /// Byte offset at which the line begins in the complete snippet.
    start: usize,
    /// Leading spaces or tabs before the comment text.
    indentation: &'source str,
    /// Non-indentation line content without its trailing newline.
    content: &'source str,
}

impl<'source> TemplateSourceLine<'source> {
    /// Splits an entire source snippet into position-aware lines.
    fn parse_all(source: &'source str) -> Vec<Self> {
        let mut start = 0;
        let source_lines = source.split_inclusive('\n').map(|line| {
            let result = Self::parse(line, start);
            start += line.len();
            result
        });
        source_lines.collect()
    }

    /// Parses one newline-terminated or final source line.
    fn parse(line: &'source str, start: usize) -> Self {
        let body = line.strip_suffix('\n').unwrap_or(line);
        let indentation_end = Self::indentation_end(body);
        Self {
            start,
            indentation: &body[..indentation_end],
            content: &body[indentation_end..],
        }
    }

    /// Returns the byte position of the first non-indentation character.
    fn indentation_end(body: &str) -> usize {
        body.char_indices()
            .find_map(|(index, character)| (!matches!(character, ' ' | '\t')).then_some(index))
            .unwrap_or(body.len())
    }
}

/// Required placeholder through which a divider template captures section content.
const TEMPLATE_CONTENT_PLACEHOLDER: &str = "{content}";

/// Validated multiline divider template used for matching and rendering.
struct Template {
    /// Original template source retained for exact rendering.
    source: String,
    /// Parsed static fragments for each template line.
    lines: Vec<TemplateLine>,
}

impl Template {
    /// Validates and parses a configured divider template.
    fn parse(source: &str, max_line_length: usize) -> Result<Self, String> {
        // Reject invalid source framing and unusable configured widths.
        if source.starts_with('\n') || source.ends_with('\n') || source.contains('\r') {
            return Err(
                "template must not have leading, trailing, or carriage-return newlines".to_owned(),
            );
        }
        if max_line_length == 0 {
            return Err("max_line_length must be greater than zero".to_owned());
        }

        // Require exactly one semantic content capture across all template lines.
        if source.matches(TEMPLATE_CONTENT_PLACEHOLDER).count() != 1 {
            return Err("template must contain exactly one `{content}` placeholder".to_owned());
        }

        // Parse every static template line under the configured width.
        let lines = source
            .split('\n')
            .map(|line| Self::parse_line(line, max_line_length))
            .collect::<Result<Vec<_>, _>>()?;

        // Retain the original template alongside its parsed line representation.
        Ok(Self {
            source: source.to_owned(),
            lines,
        })
    }

    /// Validates one ordinary-comment template line and splits its placeholder.
    fn parse_line(line: &str, max_line_length: usize) -> Result<TemplateLine, String> {
        // Require ordinary comments whose static portion fits the configured width.
        if !line.starts_with("//") || line.starts_with("///") || line.starts_with("//!") {
            return Err("every template line must be a normal `//` comment".to_owned());
        }
        let static_content = line.replace(TEMPLATE_CONTENT_PLACEHOLDER, "");
        if static_content.chars().count() > max_line_length {
            return Err("a static template line exceeds max_line_length".to_owned());
        }

        // Retain the static fragments surrounding the optional placeholder.
        let (before, after) = line.split_once(TEMPLATE_CONTENT_PLACEHOLDER).map_or_else(
            || (line.to_owned(), None),
            |(before, after)| (before.to_owned(), Some(after.to_owned())),
        );
        Ok(TemplateLine { before, after })
    }

    /// Matches one source line and returns any placeholder content it captures.
    fn line_content<'source>(
        template: &TemplateLine,
        line: &TemplateSourceLine<'source>,
    ) -> Result<Option<&'source str>, ()> {
        let Some(after) = &template.after else {
            return (line.content == template.before).then_some(None).ok_or(());
        };
        let rest = line.content.strip_prefix(&template.before).ok_or(())?;
        let content = rest.strip_suffix(after).ok_or(())?;
        Ok(Some(content))
    }

    /// Renders the template with `content` substituted exactly once.
    fn render(&self, content: &str) -> String {
        self.source.replace(TEMPLATE_CONTENT_PLACEHOLDER, content)
    }

    /// Attempts to match a complete consistently indented template at `index`.
    fn match_at(&self, lines: &[TemplateSourceLine<'_>], index: usize) -> Option<TemplateMatch> {
        // Match every configured line under one indentation and content capture.
        let indentation = lines[index].indentation;
        let mut content = None;
        for (offset, template) in self.lines.iter().enumerate() {
            let line = &lines[index + offset];
            if line.indentation != indentation {
                return None;
            }
            let Some(value) = Self::line_content(template, line).ok()? else {
                continue;
            };
            content = Some(value);
        }

        // Convert the matched source lines into one replacement-ready extent.
        let first = &lines[index];
        let last = &lines[index + self.lines.len() - 1];

        // Capture both the semantic content and its complete authored span.
        let content = content
            .expect("validated template has one placeholder")
            .to_owned();

        // Retain the complete source extent and captured semantic content.
        Some(TemplateMatch {
            start: first.start + indentation.len(),
            end: last.start + last.indentation.len() + last.content.len(),
            content,
            indentation: indentation.to_owned(),
        })
    }

    /// Finds non-overlapping divider templates in a source snippet.
    fn find_matches(&self, source: &str) -> Vec<TemplateMatch> {
        let lines = TemplateSourceLine::parse_all(source);
        let mut matches = Vec::new();
        let mut index = 0;
        while index + self.lines.len() <= lines.len() {
            let Some(found_match) = self.match_at(&lines, index) else {
                index += 1;
                continue;
            };

            // Advance by the whole template after recording a non-overlapping match.
            matches.push(found_match);
            index += self.lines.len();
        }
        matches
    }
}

#[cfg(test)]
mod tests {
    use super::{ParsedContent, Template};
    use crate::utils::identifier_case;

    #[test]
    fn validates_templates() {
        assert!(Template::parse("// --- {content}", 80).is_ok());
        assert!(Template::parse("// ---\n// {content}\n// ---", 80).is_ok());
        assert!(Template::parse("/* {content} */", 80).is_err());
        assert!(Template::parse("// no placeholder", 80).is_err());
        assert!(Template::parse("// {content} {content}", 80).is_err());
        assert!(Template::parse("// {content}", 0).is_err());
        assert!(Template::parse("// static line wider than configured\n// {content}", 20).is_err());
    }

    #[test]
    fn supports_custom_multiline_templates() {
        let template = Template::parse(
            "// ====================\n// {content}\n// ====================",
            48,
        )
        .expect("custom template should be valid");
        let rendered = template.render("Request: Request handling");

        assert_eq!(
            rendered,
            "// ====================\n// Request: Request handling\n// ===================="
        );
        assert_eq!(template.find_matches(&rendered).len(), 1);
        assert!(rendered.lines().all(|line| line.chars().count() <= 48));
    }

    #[test]
    fn finds_indented_templates_in_inline_modules() {
        let template = Template::parse(
            "// -----------------------------------------------------------------------------\n// {content}\n// -----------------------------------------------------------------------------",
            80,
        )
        .expect("default template should be valid");
        let source = concat!(
            "\n    // -----------------------------------------------------------------------------\n",
            "    // Request\n",
            "    // -----------------------------------------------------------------------------\n\n    "
        );

        assert_eq!(template.find_matches(source).len(), 1);
    }

    #[test]
    fn finds_longest_pascal_case_word_prefix() {
        assert_eq!(
            identifier_case::longest_common_pascal_prefix(&["Candidate", "CandidateBindingUse"]),
            Some("Candidate".to_owned())
        );
        assert_eq!(
            identifier_case::longest_common_pascal_prefix(&["MyThing", "MyOther"]),
            Some("My".to_owned())
        );
        assert_eq!(
            identifier_case::longest_common_pascal_prefix(&["MyThing", "Model"]),
            None
        );
    }

    #[test]
    fn normalizes_safe_content_errors() {
        let parsed = ParsedContent::from("Candidate : collected state");
        assert!(parsed.error.is_some());
        assert_eq!(
            parsed.normalized.as_deref(),
            Some("Candidate: Collected state")
        );
    }
}
