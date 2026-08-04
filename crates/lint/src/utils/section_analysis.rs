extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_span;

use std::collections::HashMap;

use rustc_hir::{HirId, Item, ItemKind, Mod, Node};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty;
use rustc_span::{BytePos, Span};
use serde::Deserialize;

use super::identifier_case::{
    identifier_is_pascal_case, identifier_longest_pascal_prefix, sentence_case,
};
use super::source_organization::{
    SectionAnalysis, SectionFinding, SectionGroup, SectionParticipant,
};

// -----------------------------------------------------------------------------
// Config: Shared section-divider configuration
// -----------------------------------------------------------------------------

/// Shared configuration for the section-divider lint family.
#[derive(Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Config {
    template: String,
    max_line_length: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_line_length: 80,
            template: concat!(
                "// -----------------------------------------------------------------------------\n",
                "// {content}\n",
                "// -----------------------------------------------------------------------------"
            )
            .to_owned(),
        }
    }
}

const CONFIG_KEY: &str = env!("CARGO_PKG_NAME");

// -----------------------------------------------------------------------------
// LibraryConfig: Top-level lint configuration
// -----------------------------------------------------------------------------

/// Top-level library configuration read from the `rlib-lint` table.
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct LibraryConfig {
    section_dividers: Config,
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
        let config =
            dylint_linting::config_or_default::<LibraryConfig>(CONFIG_KEY).section_dividers;
        let template = Template::parse(&config.template, config.max_line_length)
            .unwrap_or_else(|message| panic!("invalid section divider template: {message}"));
        Self {
            template,
            max_line_length: config.max_line_length,
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

    fn dividers_in_span(&self, cx: &LateContext<'_>, span: Span) -> Vec<SectionDividerOccurrence> {
        let Ok(source) = cx.sess().source_map().span_to_snippet(span) else {
            return Vec::new();
        };
        self.template
            .find_matches(&source)
            .into_iter()
            .map(|matched| {
                let start =
                    u32::try_from(matched.start).expect("source span offset should fit in BytePos");
                let end =
                    u32::try_from(matched.end).expect("source span offset should fit in BytePos");
                SectionDividerOccurrence {
                    span: span
                        .with_lo(span.lo() + BytePos(start))
                        .with_hi(span.lo() + BytePos(end)),
                    raw_content: matched.content,
                    indentation: matched.indentation,
                }
            })
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
            self.template
                .render(content)
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
    fn analyze(
        analyzer: &SectionAnalyzer,
        cx: &LateContext<'_>,
        module: &Mod<'_>,
        hir_id: HirId,
    ) -> SectionAnalysis {
        let source_map = cx.sess().source_map();
        let module_span = Self::source_span(cx, module, hir_id);
        let mut items = module
            .item_ids
            .iter()
            .map(|item_id| cx.tcx.hir_item(*item_id))
            .filter(|item| {
                !item.span.from_expansion()
                    && source_map.span_to_filename(item.span)
                        == source_map.span_to_filename(module_span)
            })
            .collect::<Vec<_>>();
        items.sort_unstable_by_key(|item| item.span.lo());

        let mut events = Vec::new();
        let mut previous = module_span.lo();
        for item in items {
            if previous <= item.span.lo() {
                let gap = module_span.with_lo(previous).with_hi(item.span.lo());
                events.extend(
                    analyzer
                        .dividers_in_span(cx, gap)
                        .into_iter()
                        .map(SectionEvent::SectionDividerOccurrence),
                );
            }
            if let Some(participant) = SectionCandidate::from_item(cx, item) {
                events.push(SectionEvent::SectionCandidate(participant));
            }
            previous = previous.max(item.span.hi());
        }
        if previous <= module_span.hi() {
            let gap = module_span.with_lo(previous).with_hi(module_span.hi());
            events.extend(
                analyzer
                    .dividers_in_span(cx, gap)
                    .into_iter()
                    .map(SectionEvent::SectionDividerOccurrence),
            );
        }
        events.sort_unstable_by_key(SectionEvent::position);
        Self::analyze_events(analyzer, events)
    }

    fn analyze_events(analyzer: &SectionAnalyzer, events: Vec<SectionEvent>) -> SectionAnalysis {
        let mut analysis = SectionAnalysis::default();
        let mut uncovered = SectionCandidateList::default();
        let mut current: Option<Section> = None;
        let mut seen_prefixes = HashMap::<String, Span>::new();

        for event in events {
            match event {
                SectionEvent::SectionCandidate(participant) => {
                    if let Some(section) = &mut current {
                        section.participants.push(participant);
                    } else {
                        uncovered.push(participant);
                    }
                }
                SectionEvent::SectionDividerOccurrence(divider) => {
                    if uncovered.requires_divider() {
                        analysis.missing.push(uncovered.missing_finding());
                    }
                    uncovered.clear();
                    if let Some(section) = current.take() {
                        Self::finish_section(analyzer, &section, &mut seen_prefixes, &mut analysis);
                    }
                    current = Some(Section {
                        divider,
                        participants: SectionCandidateList::default(),
                    });
                }
            }
        }

        if uncovered.requires_divider() {
            analysis.missing.push(uncovered.missing_finding());
        }
        if let Some(section) = current {
            Self::finish_section(analyzer, &section, &mut seen_prefixes, &mut analysis);
        }
        analysis
    }

    fn source_span(cx: &LateContext<'_>, module: &Mod<'_>, hir_id: HirId) -> Span {
        let inner = module.spans.inner_span;
        let item = [cx.tcx.hir_node(hir_id), cx.tcx.parent_hir_node(hir_id)]
            .into_iter()
            .find_map(|node| match node {
                Node::Item(item) if matches!(item.kind, ItemKind::Mod(..)) => Some(item),
                _ => None,
            });
        let Some(item) = item else {
            return inner;
        };
        if cx.sess().source_map().span_to_filename(item.span)
            != cx.sess().source_map().span_to_filename(inner)
        {
            return inner;
        }

        let source_map = cx.sess().source_map();
        let lo = source_map
            .span_to_snippet(item.span.with_hi(inner.lo()))
            .ok()
            .and_then(|source| source.rfind('{'))
            .and_then(|offset| u32::try_from(offset + 1).ok())
            .map_or(inner.lo(), |offset| item.span.lo() + BytePos(offset));
        let hi = source_map
            .span_to_snippet(item.span.with_lo(inner.hi()))
            .ok()
            .and_then(|source| source.find('}'))
            .and_then(|offset| u32::try_from(offset).ok())
            .map_or(inner.hi(), |offset| inner.hi() + BytePos(offset));
        inner.with_lo(lo).with_hi(hi)
    }

    fn finish_section(
        analyzer: &SectionAnalyzer,
        section: &Section,
        seen_prefixes: &mut HashMap<String, Span>,
        analysis: &mut SectionAnalysis,
    ) {
        let parsed = ParsedContent::parse(&section.divider.raw_content);
        let prefix = parsed.prefix.clone();

        if section.participants.is_empty() {
            analysis.malformed.push(SectionFinding {
                span: section.divider.span,
                message: "section divider does not contain any module declarations".to_owned(),
                help: "remove this divider or place it before the declarations it describes"
                    .to_owned(),
                replacement: None,
            });
        }

        if let Some(message) = &parsed.error {
            let replacement = parsed.normalized.as_ref().and_then(|content| {
                analyzer.render_replacement(content, &section.divider.indentation)
            });
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

        if let Some(prefix) = prefix {
            if parsed.error.is_none()
                && analyzer.rendered_lines_fit(&section.divider.raw_content)
                && !section.participants.is_empty()
            {
                analysis.sections.push(SectionGroup {
                    ordinal: analysis.sections.len() + 1,
                    prefix: prefix.clone(),
                    span: section.divider.span,
                    participants: section.participants.distinct_declarations(),
                });
            }
            if seen_prefixes
                .insert(prefix.clone(), section.divider.span)
                .is_some()
            {
                analysis.duplicates.push(SectionFinding {
                    span: section.divider.span,
                    message: format!(
                        "section prefix `{prefix}` is used more than once in this module"
                    ),
                    help: format!("combine this family with the earlier `{prefix}` section"),
                    replacement: None,
                });
            }

            if !section.participants.is_empty() {
                let names = section.participants.names();
                match identifier_longest_pascal_prefix(&names) {
                Some(expected) if expected != prefix => {
                    analysis.mismatches.push(SectionFinding {
                        span: section.divider.span,
                        message: format!(
                            "section prefix `{prefix}` does not match its declaration family"
                        ),
                        help: format!(
                            "rename the declarations into one coherent family first; their longest shared PascalCase prefix is `{expected}`, and a new section is appropriate only for an independent concept"
                        ),
                        replacement: None,
                    });
                }
                None => analysis.mismatches.push(SectionFinding {
                    span: section.divider.span,
                    message: format!(
                        "section `{prefix}` contains declarations without a shared PascalCase prefix"
                    ),
                    help: format!(
                        "reconsider the names {} so closely related declarations share a visible prefix; split the section only when they represent independent concepts",
                        section.participants.formatted_names()
                    ),
                    replacement: None,
                }),
                _ => {}
            }
            }
        }
    }
}

// -----------------------------------------------------------------------------
// Section: Section events and participants
// -----------------------------------------------------------------------------

struct SectionDividerOccurrence {
    span: Span,
    raw_content: String,
    indentation: String,
}

struct SectionCandidate {
    def_id: rustc_hir::def_id::LocalDefId,
    name: String,
    span: Span,
    is_nominal_declaration: bool,
}

impl SectionCandidate {
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
            ItemKind::Impl(_) => {
                let self_type = cx.tcx.type_of(item.owner_id).instantiate_identity();
                let ty::Adt(definition, _) = self_type.kind() else {
                    return None;
                };
                definition.did().as_local().map(|definition| Self {
                    def_id: definition,
                    name: cx.tcx.item_name(definition.to_def_id()).to_string(),
                    span: item.span,
                    is_nominal_declaration: false,
                })
            }
            _ => None,
        }
    }
}

enum SectionEvent {
    SectionDividerOccurrence(SectionDividerOccurrence),
    SectionCandidate(SectionCandidate),
}

impl SectionEvent {
    fn position(&self) -> BytePos {
        match self {
            Self::SectionDividerOccurrence(divider) => divider.span.lo(),
            Self::SectionCandidate(participant) => participant.span.lo(),
        }
    }
}

#[derive(Default)]
struct SectionCandidateList(Vec<SectionCandidate>);

impl SectionCandidateList {
    fn clear(&mut self) {
        self.0.clear();
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn push(&mut self, participant: SectionCandidate) {
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
        names
            .into_iter()
            .map(|name| format!("`{name}`"))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn distinct_declarations(&self) -> Vec<SectionParticipant> {
        let mut seen = std::collections::HashSet::new();
        self.0
            .iter()
            .filter(|participant| seen.insert(participant.def_id))
            .map(|participant| SectionParticipant {
                def_id: participant.def_id,
                name: participant.name.clone(),
                span: participant.span,
                is_nominal: participant.is_nominal_declaration,
            })
            .collect()
    }

    fn missing_finding(&self) -> SectionFinding {
        let names = self.names();
        let guidance = identifier_longest_pascal_prefix(&names).map_or_else(
            || format!(
                "reconsider the names {} so related declarations share a visible prefix, then add a divider; create separate sections only for independent concepts",
                self.formatted_names()
            ),
            |prefix| format!(
                "add a divider for `{prefix}`, after first checking whether any outlier should be renamed into that family"
            ),
        );
        SectionFinding {
            span: self.0[0].span,
            message: "module declarations are not covered by a section divider".to_owned(),
            help: guidance,
            replacement: None,
        }
    }
}

struct Section {
    divider: SectionDividerOccurrence,
    participants: SectionCandidateList,
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
        let prefix = identifier_is_pascal_case(raw_prefix).then(|| raw_prefix.to_owned());
        let normalized_description = raw_description.map(sentence_case);
        let normalized = prefix
            .as_ref()
            .and_then(|prefix| match &normalized_description {
                Some(description) if !description.is_empty() => {
                    Some(format!("{prefix}: {description}"))
                }
                None => Some(prefix.clone()),
                _ => None,
            });

        let error = if prefix.is_none() {
            Some("section divider prefix is not PascalCase".to_owned())
        } else if raw_description.is_some_and(str::is_empty) {
            Some("section divider description is empty".to_owned())
        } else if raw_description != normalized_description.as_deref() {
            Some("section divider description must use sentence case".to_owned())
        } else if normalized.as_deref() != Some(content) {
            Some("section divider content has non-canonical spacing".to_owned())
        } else {
            None
        };

        Self {
            prefix,
            normalized,
            error,
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
        source
            .split_inclusive('\n')
            .map(|line| {
                let body = line.strip_suffix('\n').unwrap_or(line);
                let indentation_end = body
                    .char_indices()
                    .find_map(|(index, character)| {
                        (!matches!(character, ' ' | '\t')).then_some(index)
                    })
                    .unwrap_or(body.len());
                let result = Self {
                    start,
                    indentation: &body[..indentation_end],
                    content: &body[indentation_end..],
                };
                start += line.len();
                result
            })
            .collect()
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
            .map(|line| {
                if !line.starts_with("//") || line.starts_with("///") || line.starts_with("//!") {
                    return Err("every template line must be a normal `//` comment".to_owned());
                }
                if line
                    .replace(TEMPLATE_CONTENT_PLACEHOLDER, "")
                    .chars()
                    .count()
                    > max_line_length
                {
                    return Err("a static template line exceeds max_line_length".to_owned());
                }
                let (before, after) = line.split_once(TEMPLATE_CONTENT_PLACEHOLDER).map_or_else(
                    || (line.to_owned(), None),
                    |(before, after)| (before.to_owned(), Some(after.to_owned())),
                );
                Ok(TemplateLine { before, after })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            source: source.to_owned(),
            lines,
        })
    }

    fn render(&self, content: &str) -> String {
        self.source.replace(TEMPLATE_CONTENT_PLACEHOLDER, content)
    }

    fn find_matches(&self, source: &str) -> Vec<TemplateMatch> {
        let lines = TemplateSourceLine::parse_all(source);
        let mut matches = Vec::new();
        let mut index = 0;
        while index + self.lines.len() <= lines.len() {
            let indentation = lines[index].indentation;
            let mut content = None;
            let mut valid = true;
            for (offset, template) in self.lines.iter().enumerate() {
                let line = &lines[index + offset];
                if line.indentation != indentation {
                    valid = false;
                    break;
                }
                match &template.after {
                    None if line.content != template.before => {
                        valid = false;
                        break;
                    }
                    None => {}
                    Some(after) => {
                        let Some(rest) = line.content.strip_prefix(&template.before) else {
                            valid = false;
                            break;
                        };
                        let Some(value) = rest.strip_suffix(after) else {
                            valid = false;
                            break;
                        };
                        content = Some(value.to_owned());
                    }
                }
            }
            if valid {
                let first = &lines[index];
                let last = &lines[index + self.lines.len() - 1];
                matches.push(TemplateMatch {
                    start: first.start + indentation.len(),
                    end: last.start + last.indentation.len() + last.content.len(),
                    content: content.expect("validated template has one placeholder"),
                    indentation: indentation.to_owned(),
                });
                index += self.lines.len();
            } else {
                index += 1;
            }
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
