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
use super::identifier_case::{
    identifier_is_pascal_case, identifier_longest_pascal_prefix, sentence_case,
};

// -----------------------------------------------------------------------------
// Section: Shared organization analysis data
// -----------------------------------------------------------------------------

/// One source-level problem found by the shared analyzer.
pub(crate) struct SectionFinding {
    pub(crate) span: Span,
    pub(crate) message: String,
    pub(crate) help: String,
    pub(crate) replacement: Option<String>,
}

/// One distinct declaration covered by a valid section.
pub(crate) struct SectionParticipant {
    pub(crate) def_id: rustc_hir::def_id::LocalDefId,
    pub(crate) name: String,
    pub(crate) span: Span,
    pub(crate) is_nominal: bool,
}

/// One valid authored section available to semantic companion lints.
pub(crate) struct SectionGroup {
    pub(crate) ordinal: usize,
    pub(crate) prefix: String,
    pub(crate) span: Span,
    pub(crate) participants: Vec<SectionParticipant>,
}

/// Findings split by lint identity so each rule remains independently configurable.
#[derive(Default)]
pub(crate) struct SectionAnalysis {
    pub(crate) missing: Vec<SectionFinding>,
    pub(crate) malformed: Vec<SectionFinding>,
    pub(crate) duplicates: Vec<SectionFinding>,
    pub(crate) mismatches: Vec<SectionFinding>,
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

/// Parses configured divider blocks and relates them to module-level declaration groups.
pub(crate) struct SectionAnalyzer {
    template: Template,
    max_line_length: usize,
}

impl SectionAnalyzer {
    /// Loads and validates the shared library configuration.
    pub(crate) fn from_config() -> Self {
        let config = LibraryConfig::load().section_dividers;

        // Fail during lint construction when the project template is invalid.
        let parsed_template = Template::parse(&config.template, config.max_line_length);
        let template = parsed_template
            .unwrap_or_else(|message| panic!("invalid section divider template: {message}"));
        Self {
            template,
            max_line_length: config.max_line_length,
        }
    }

    fn divider_from_match(span: Span, matched: TemplateMatch) -> SectionEventDivider {
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

    fn rendered_lines_fit(&self, content: &str) -> bool {
        self.template
            .render(content)
            .lines()
            .all(|line| line.chars().count() <= self.max_line_length)
    }

    fn render_replacement(&self, content: &str, indentation: &str) -> Option<String> {
        self.rendered_lines_fit(content).then(|| {
            let rendered = self.template.render(content);
            rendered
                .lines()
                .collect::<Vec<_>>()
                .join(&format!("\n{indentation}"))
        })
    }
}

// -----------------------------------------------------------------------------
// ModuleAnalysis: Complete module analysis
// -----------------------------------------------------------------------------

struct ModuleAnalysis;

impl ModuleAnalysis {
    fn append_dividers(
        analyzer: &SectionAnalyzer,
        cx: &LateContext<'_>,
        span: Span,
        events: &mut Vec<SectionEvent>,
    ) {
        let dividers = analyzer.dividers_in_span(cx, span);
        events.extend(dividers.into_iter().map(SectionEvent::Divider));
    }

    fn analyze(
        analyzer: &SectionAnalyzer,
        cx: &LateContext<'_>,
        module: &Mod<'_>,
        hir_id: HirId,
    ) -> SectionAnalysis {
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
                events.push(SectionEvent::Candidate(participant));
            }
            previous = previous.max(item.span.hi());
        }
        if previous <= module_span.hi() {
            let gap = module_span.with_lo(previous).with_hi(module_span.hi());
            Self::append_dividers(analyzer, cx, gap, &mut events);
        }
        events.sort_unstable_by_key(SectionEvent::position);
        Self::analyze_events(analyzer, events)
    }

    fn analyze_events(analyzer: &SectionAnalyzer, events: Vec<SectionEvent>) -> SectionAnalysis {
        let mut state = SectionEventAnalysisState::default();

        // Close each group when the following divider starts a new one.
        for event in events {
            state.apply(analyzer, event);
        }
        state.finish(analyzer)
    }

    fn source_span(cx: &LateContext<'_>, module: &Mod<'_>, hir_id: HirId) -> Span {
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
        let lo = opening_offset.map_or(inner.lo(), |offset| item.span.lo() + offset);

        let closing_source = source_map.span_to_snippet(item.span.with_lo(inner.hi()));
        let closing_brace = closing_source.ok().and_then(|source| source.find('}'));
        let closing_offset = closing_brace
            .and_then(|offset| u32::try_from(offset).ok())
            .map(BytePos);
        let hi = closing_offset.map_or(inner.hi(), |offset| inner.hi() + offset);
        inner.with_lo(lo).with_hi(hi)
    }

    fn record_malformed_section(
        analyzer: &SectionAnalyzer,
        section: &SectionEventGroup,
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
            let replacement = parsed.normalized.as_ref().and_then(|content| {
                analyzer.render_replacement(content, &section.divider.indentation)
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
            analysis.malformed.push(SectionFinding {
                span: section.divider.span,
                message: format!(
                    "section divider exceeds the configured {}-character line limit",
                    analyzer.max_line_length
                ),
                help: "shorten the optional description or choose a more compact template"
                    .to_owned(),
                replacement: None,
            });
        }
    }

    fn record_valid_section(
        analyzer: &SectionAnalyzer,
        section: &SectionEventGroup,
        parsed: &ParsedContent,
        prefix: &str,
        analysis: &mut SectionAnalysis,
    ) {
        let is_valid = parsed.error.is_none()
            && analyzer.rendered_lines_fit(&section.divider.raw_content)
            && !section.participants.is_empty();
        if !is_valid {
            return;
        }
        analysis.sections.push(SectionGroup {
            ordinal: analysis.sections.len() + 1,
            prefix: prefix.to_owned(),
            span: section.divider.span,
            participants: section.participants.distinct_declarations(),
        });
    }

    fn record_duplicate_section(
        section: &SectionEventGroup,
        prefix: &str,
        seen_prefixes: &mut HashMap<String, Span>,
        analysis: &mut SectionAnalysis,
    ) {
        if seen_prefixes
            .insert(prefix.to_owned(), section.divider.span)
            .is_none()
        {
            return;
        }
        analysis.duplicates.push(SectionFinding {
            span: section.divider.span,
            message: format!("section prefix `{prefix}` is used more than once in this module"),
            help: format!("combine this family with the earlier `{prefix}` section"),
            replacement: None,
        });
    }

    fn wrong_prefix_finding(
        section: &SectionEventGroup,
        prefix: &str,
        expected: &str,
    ) -> SectionFinding {
        SectionFinding {
            span: section.divider.span,
            message: format!("section prefix `{prefix}` does not match its declaration family"),
            help: format!(
                "rename the declarations into one coherent family first; their longest shared PascalCase prefix is `{expected}`, and a new section is appropriate only for an independent concept"
            ),
            replacement: None,
        }
    }

    fn unrelated_names_finding(section: &SectionEventGroup, prefix: &str) -> SectionFinding {
        SectionFinding {
            span: section.divider.span,
            message: format!(
                "section `{prefix}` contains declarations without a shared PascalCase prefix"
            ),
            help: format!(
                "reconsider the names {} so closely related declarations share a visible prefix; split the section only when they represent independent concepts",
                section.participants.formatted_names()
            ),
            replacement: None,
        }
    }

    fn mismatch_finding(section: &SectionEventGroup, prefix: &str) -> Option<SectionFinding> {
        if section.participants.is_empty() {
            return None;
        }
        let names = section.participants.names();
        match identifier_longest_pascal_prefix(&names) {
            Some(expected) if expected != prefix => {
                Some(Self::wrong_prefix_finding(section, prefix, &expected))
            }
            None => Some(Self::unrelated_names_finding(section, prefix)),
            _ => None,
        }
    }

    fn finish_section(
        analyzer: &SectionAnalyzer,
        section: &SectionEventGroup,
        seen_prefixes: &mut HashMap<String, Span>,
        analysis: &mut SectionAnalysis,
    ) {
        let parsed = ParsedContent::parse(&section.divider.raw_content);
        Self::record_malformed_section(analyzer, section, &parsed, analysis);
        let Some(prefix) = parsed.prefix.as_deref() else {
            return;
        };

        // Record the section independently for each semantic companion lint.
        Self::record_valid_section(analyzer, section, &parsed, prefix, analysis);
        Self::record_duplicate_section(section, prefix, seen_prefixes, analysis);
        let Some(finding) = Self::mismatch_finding(section, prefix) else {
            return;
        };
        analysis.mismatches.push(finding);
    }
}

// -----------------------------------------------------------------------------
// SectionEvent: Section events and participants
// -----------------------------------------------------------------------------

struct SectionEventDivider {
    span: Span,
    raw_content: String,
    indentation: String,
}

struct SectionEventCandidate {
    def_id: rustc_hir::def_id::LocalDefId,
    name: String,
    span: Span,
    is_nominal_declaration: bool,
}

impl SectionEventCandidate {
    fn from_impl(cx: &LateContext<'_>, item: &Item<'_>) -> Option<Self> {
        let self_type = cx.tcx.type_of(item.owner_id).instantiate_identity();
        let ty::Adt(definition, _) = self_type.kind() else {
            return None;
        };
        let definition = definition.did().as_local()?;
        Some(Self {
            def_id: definition,
            name: cx.tcx.item_name(definition.to_def_id()).to_string(),
            span: item.span,
            is_nominal_declaration: false,
        })
    }

    fn from_item(cx: &LateContext<'_>, item: &Item<'_>) -> Option<Self> {
        if item.span.from_expansion() {
            return None;
        }
        match item.kind {
            ItemKind::Struct(..)
            | ItemKind::Enum(..)
            | ItemKind::Union(..)
            | ItemKind::TyAlias(..)
            | ItemKind::Trait(..)
            | ItemKind::TraitAlias(..) => Some(Self {
                def_id: item.owner_id.def_id,
                name: cx.tcx.item_name(item.owner_id.to_def_id()).to_string(),
                span: item.span,
                is_nominal_declaration: true,
            }),
            ItemKind::Const(..) | ItemKind::Static(..) | ItemKind::Fn { .. } => Some(Self {
                def_id: item.owner_id.def_id,
                name: cx.tcx.item_name(item.owner_id.to_def_id()).to_string(),
                span: item.span,
                is_nominal_declaration: false,
            }),
            ItemKind::Impl(_) => Self::from_impl(cx, item),
            _ => None,
        }
    }
}

enum SectionEvent {
    Divider(SectionEventDivider),
    Candidate(SectionEventCandidate),
}

impl SectionEvent {
    fn position(&self) -> BytePos {
        match self {
            Self::Divider(divider) => divider.span.lo(),
            Self::Candidate(participant) => participant.span.lo(),
        }
    }
}

#[derive(Default)]
struct SectionEventCandidates(Vec<SectionEventCandidate>);

impl SectionEventCandidates {
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

    fn clear(&mut self) {
        self.0.clear();
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn push(&mut self, participant: SectionEventCandidate) {
        self.0.push(participant);
    }

    fn requires_divider(&self) -> bool {
        self.0.len() > 1
            || self
                .0
                .iter()
                .any(|participant| participant.is_nominal_declaration)
    }

    fn names(&self) -> Vec<&str> {
        self.0
            .iter()
            .map(|participant| participant.name.as_str())
            .collect()
    }

    fn formatted_names(&self) -> String {
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

    fn missing_guidance(&self) -> String {
        let names = self.names();

        // Turn the inferred prefix, or its absence, into naming-first guidance.
        identifier_longest_pascal_prefix(&names).map_or_else(
            || format!(
                "reconsider the names {} so related declarations share a visible prefix, then add a divider; create separate sections only for independent concepts",
                self.formatted_names()
            ),
            |prefix| format!(
                "add a divider for `{prefix}`, after first checking whether any outlier should be renamed into that family"
            ),
        )
    }

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

struct SectionEventGroup {
    divider: SectionEventDivider,
    participants: SectionEventCandidates,
}

#[derive(Default)]
struct SectionEventAnalysisState {
    analysis: SectionAnalysis,
    uncovered: SectionEventCandidates,
    current: Option<SectionEventGroup>,
    seen_prefixes: HashMap<String, Span>,
}

impl SectionEventAnalysisState {
    fn record_candidate(&mut self, participant: SectionEventCandidate) {
        let Some(section) = &mut self.current else {
            self.uncovered.push(participant);
            return;
        };
        section.participants.push(participant);
    }

    fn record_uncovered(&mut self) {
        if self.uncovered.requires_divider() {
            self.analysis.missing.push(self.uncovered.missing_finding());
        }
        self.uncovered.clear();
    }

    fn start_section(&mut self, analyzer: &SectionAnalyzer, divider: SectionEventDivider) {
        self.record_uncovered();

        // Finish the preceding section before installing its successor.
        if let Some(section) = self.current.take() {
            ModuleAnalysis::finish_section(
                analyzer,
                &section,
                &mut self.seen_prefixes,
                &mut self.analysis,
            );
        }
        self.current = Some(SectionEventGroup {
            divider,
            participants: SectionEventCandidates::default(),
        });
    }

    fn apply(&mut self, analyzer: &SectionAnalyzer, event: SectionEvent) {
        match event {
            SectionEvent::Candidate(participant) => self.record_candidate(participant),
            SectionEvent::Divider(divider) => self.start_section(analyzer, divider),
        }
    }

    fn finish(mut self, analyzer: &SectionAnalyzer) -> SectionAnalysis {
        self.record_uncovered();
        let Some(section) = self.current.take() else {
            return self.analysis;
        };

        // Close the final authored section after the event stream ends.
        ModuleAnalysis::finish_section(
            analyzer,
            &section,
            &mut self.seen_prefixes,
            &mut self.analysis,
        );
        self.analysis
    }
}

// -----------------------------------------------------------------------------
// ParsedContent: Parsed divider content
// -----------------------------------------------------------------------------

struct ParsedContent {
    prefix: Option<String>,
    normalized: Option<String>,
    error: Option<String>,
}

impl ParsedContent {
    fn parse(content: &str) -> Self {
        let trimmed = content.trim();
        let (raw_prefix, raw_description) = trimmed
            .split_once(':')
            .map_or((trimmed, None), |(prefix, description)| {
                (prefix.trim(), Some(description.trim()))
            });

        // Normalize both halves before classifying the first canonical-form violation.
        let prefix = identifier_is_pascal_case(raw_prefix).then(|| raw_prefix.to_owned());
        let normalized_description = raw_description.map(sentence_case);
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

        Self {
            prefix,
            normalized,
            error,
        }
    }

    fn normalized_content(prefix: Option<&str>, description: Option<&str>) -> Option<String> {
        match (prefix, description) {
            (Some(prefix), Some(description)) if !description.is_empty() => {
                Some(format!("{prefix}: {description}"))
            }
            (Some(prefix), None) => Some(prefix.to_owned()),
            _ => None,
        }
    }

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

struct TemplateLine {
    before: String,
    after: Option<String>,
}

struct TemplateMatch {
    start: usize,
    end: usize,
    content: String,
    indentation: String,
}

struct TemplateSourceLine<'source> {
    start: usize,
    indentation: &'source str,
    content: &'source str,
}

impl<'source> TemplateSourceLine<'source> {
    fn parse_all(source: &'source str) -> Vec<Self> {
        let mut start = 0;
        let source_lines = source.split_inclusive('\n').map(|line| {
            let result = Self::parse(line, start);
            start += line.len();
            result
        });
        source_lines.collect()
    }

    fn parse(line: &'source str, start: usize) -> Self {
        let body = line.strip_suffix('\n').unwrap_or(line);
        let indentation_end = Self::indentation_end(body);
        Self {
            start,
            indentation: &body[..indentation_end],
            content: &body[indentation_end..],
        }
    }

    fn indentation_end(body: &str) -> usize {
        body.char_indices()
            .find_map(|(index, character)| (!matches!(character, ' ' | '\t')).then_some(index))
            .unwrap_or(body.len())
    }
}

const TEMPLATE_CONTENT_PLACEHOLDER: &str = "{content}";

struct Template {
    source: String,
    lines: Vec<TemplateLine>,
}

impl Template {
    fn parse(source: &str, max_line_length: usize) -> Result<Self, String> {
        if source.starts_with('\n') || source.ends_with('\n') || source.contains('\r') {
            return Err(
                "template must not have leading, trailing, or carriage-return newlines".to_owned(),
            );
        }
        if max_line_length == 0 {
            return Err("max_line_length must be greater than zero".to_owned());
        }
        if source.matches(TEMPLATE_CONTENT_PLACEHOLDER).count() != 1 {
            return Err("template must contain exactly one `{content}` placeholder".to_owned());
        }
        let lines = source
            .split('\n')
            .map(|line| Self::parse_line(line, max_line_length))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            source: source.to_owned(),
            lines,
        })
    }

    fn parse_line(line: &str, max_line_length: usize) -> Result<TemplateLine, String> {
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

    fn render(&self, content: &str) -> String {
        self.source.replace(TEMPLATE_CONTENT_PLACEHOLDER, content)
    }

    fn match_at(&self, lines: &[TemplateSourceLine<'_>], index: usize) -> Option<TemplateMatch> {
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
        Some(TemplateMatch {
            start: first.start + indentation.len(),
            end: last.start + last.indentation.len() + last.content.len(),
            content: content
                .expect("validated template has one placeholder")
                .to_owned(),
            indentation: indentation.to_owned(),
        })
    }

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
    use crate::utils::identifier_case::identifier_longest_pascal_prefix;

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
            identifier_longest_pascal_prefix(&["Candidate", "CandidateBindingUse"]),
            Some("Candidate".to_owned())
        );
        assert_eq!(
            identifier_longest_pascal_prefix(&["MyThing", "MyOther"]),
            Some("My".to_owned())
        );
        assert_eq!(
            identifier_longest_pascal_prefix(&["MyThing", "Model"]),
            None
        );
    }

    #[test]
    fn normalizes_safe_content_errors() {
        let parsed = ParsedContent::parse("Candidate : collected state");
        assert!(parsed.error.is_some());
        assert_eq!(
            parsed.normalized.as_deref(),
            Some("Candidate: Collected state")
        );
    }
}
