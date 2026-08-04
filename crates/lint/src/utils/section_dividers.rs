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

use std::collections::HashMap;

use rustc_hir::{Item, ItemKind, Mod};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty;
use rustc_span::{BytePos, Span};
use serde::Deserialize;

const CONFIG_KEY: &str = env!("CARGO_PKG_NAME");
const CONTENT_PLACEHOLDER: &str = "{content}";

// -----------------------------------------------------------------------------
// SectionDivider: Shared configuration
// -----------------------------------------------------------------------------

/// Shared configuration for the section-divider lint family.
#[derive(Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct SectionDividerConfig {
    template: String,
    max_line_length: usize,
}

impl Default for SectionDividerConfig {
    fn default() -> Self {
        Self {
            template: concat!(
                "// -----------------------------------------------------------------------------\n",
                "// {content}\n",
                "// -----------------------------------------------------------------------------"
            )
            .to_owned(),
            max_line_length: 80,
        }
    }
}

/// Top-level library configuration read from the `rlib-lint` table.
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct SectionDividerLibraryConfig {
    section_dividers: SectionDividerConfig,
}

// -----------------------------------------------------------------------------
// Analysis: Shared analysis results
// -----------------------------------------------------------------------------

/// One source-level problem found by the shared analyzer.
pub(crate) struct AnalysisFinding {
    pub(crate) span: Span,
    pub(crate) message: String,
    pub(crate) help: String,
    pub(crate) replacement: Option<String>,
}

/// Findings split by lint identity so each rule remains independently configurable.
#[derive(Default)]
pub(crate) struct Analysis {
    pub(crate) missing: Vec<AnalysisFinding>,
    pub(crate) malformed: Vec<AnalysisFinding>,
    pub(crate) duplicates: Vec<AnalysisFinding>,
    pub(crate) mismatches: Vec<AnalysisFinding>,
}

// -----------------------------------------------------------------------------
// DividerAnalyzer: Source analysis
// -----------------------------------------------------------------------------

/// Parses configured divider blocks and relates them to module-level type families.
pub(crate) struct DividerAnalyzer {
    template: Template,
    max_line_length: usize,
}

impl DividerAnalyzer {
    /// Loads and validates the shared library configuration.
    pub(crate) fn from_config() -> Self {
        let config = dylint_linting::config_or_default::<SectionDividerLibraryConfig>(CONFIG_KEY)
            .section_dividers;
        let template = Template::parse(&config.template, config.max_line_length)
            .unwrap_or_else(|message| panic!("invalid section divider template: {message}"));
        Self {
            template,
            max_line_length: config.max_line_length,
        }
    }

    /// Analyzes one source module without inspecting comments inside item bodies.
    pub(crate) fn analyze(&self, cx: &LateContext<'_>, module: &Mod<'_>) -> Analysis {
        let source_map = cx.sess().source_map();
        let module_span = module.spans.inner_span;
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
                    self.dividers_in_span(cx, gap)
                        .into_iter()
                        .map(SectionEvent::SectionDividerOccurrence),
                );
            }
            if let Some(name) = participant_name(cx, item) {
                events.push(SectionEvent::SectionParticipant(SectionParticipant {
                    name,
                    span: item.span,
                }));
            }
            previous = previous.max(item.span.hi());
        }
        if previous <= module_span.hi() {
            let gap = module_span.with_lo(previous).with_hi(module_span.hi());
            events.extend(
                self.dividers_in_span(cx, gap)
                    .into_iter()
                    .map(SectionEvent::SectionDividerOccurrence),
            );
        }
        events.sort_unstable_by_key(SectionEvent::position);
        self.analyze_events(events)
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

    fn analyze_events(&self, events: Vec<SectionEvent>) -> Analysis {
        let mut analysis = Analysis::default();
        let mut uncovered = SectionParticipantList::default();
        let mut current: Option<Section> = None;
        let mut seen_prefixes = HashMap::<String, Span>::new();

        for event in events {
            match event {
                SectionEvent::SectionParticipant(participant) => {
                    if let Some(section) = &mut current {
                        section.participants.push(participant);
                    } else {
                        uncovered.push(participant);
                    }
                }
                SectionEvent::SectionDividerOccurrence(divider) => {
                    if !uncovered.is_empty() {
                        analysis.missing.push(uncovered.missing_finding());
                        uncovered.clear();
                    }
                    if let Some(section) = current.take() {
                        self.finish_section(&section, &mut seen_prefixes, &mut analysis);
                    }
                    current = Some(Section {
                        divider,
                        participants: SectionParticipantList::default(),
                    });
                }
            }
        }

        if !uncovered.is_empty() {
            analysis.missing.push(uncovered.missing_finding());
        }
        if let Some(section) = current {
            self.finish_section(&section, &mut seen_prefixes, &mut analysis);
        }
        analysis
    }

    fn finish_section(
        &self,
        section: &Section,
        seen_prefixes: &mut HashMap<String, Span>,
        analysis: &mut Analysis,
    ) {
        let parsed = parse_content(&section.divider.raw_content);
        let prefix = parsed.prefix.clone();

        if section.participants.is_empty() {
            analysis.malformed.push(AnalysisFinding {
                span: section.divider.span,
                message: "section divider does not contain any type-family declarations".to_owned(),
                help: "remove this divider or place it before the declarations it describes"
                    .to_owned(),
                replacement: None,
            });
        }

        if let Some(message) = parsed.error {
            let replacement = parsed
                .normalized
                .as_ref()
                .and_then(|content| self.render_replacement(content, &section.divider.indentation));
            analysis.malformed.push(AnalysisFinding {
                span: section.divider.span,
                message,
                help: "use `PascalCasePrefix` or `PascalCasePrefix: Sentence case description`"
                    .to_owned(),
                replacement,
            });
        } else if !self.rendered_lines_fit(&section.divider.raw_content) {
            analysis.malformed.push(AnalysisFinding {
                span: section.divider.span,
                message: format!(
                    "section divider exceeds the configured {}-character line limit",
                    self.max_line_length
                ),
                help: "shorten the optional description or choose a more compact template"
                    .to_owned(),
                replacement: None,
            });
        }

        if let Some(prefix) = prefix {
            if seen_prefixes
                .insert(prefix.clone(), section.divider.span)
                .is_some()
            {
                analysis.duplicates.push(AnalysisFinding {
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
                match longest_word_prefix(&names) {
                    Some(expected) if expected != prefix => {
                        analysis.mismatches.push(AnalysisFinding {
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
                    None => analysis.mismatches.push(AnalysisFinding {
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
// Section: Section events and participants
// -----------------------------------------------------------------------------

enum SectionEvent {
    SectionDividerOccurrence(SectionDividerOccurrence),
    SectionParticipant(SectionParticipant),
}

impl SectionEvent {
    fn position(&self) -> BytePos {
        match self {
            Self::SectionDividerOccurrence(divider) => divider.span.lo(),
            Self::SectionParticipant(participant) => participant.span.lo(),
        }
    }
}

struct SectionDividerOccurrence {
    span: Span,
    raw_content: String,
    indentation: String,
}

struct SectionParticipant {
    name: String,
    span: Span,
}

#[derive(Default)]
struct SectionParticipantList(Vec<SectionParticipant>);

impl SectionParticipantList {
    fn clear(&mut self) {
        self.0.clear();
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn push(&mut self, participant: SectionParticipant) {
        self.0.push(participant);
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

    fn missing_finding(&self) -> AnalysisFinding {
        let names = self.names();
        let guidance = longest_word_prefix(&names).map_or_else(
            || format!(
                "reconsider the names {} so related declarations share a visible prefix, then add a divider; create separate sections only for independent concepts",
                self.formatted_names()
            ),
            |prefix| format!(
                "add a divider for `{prefix}`, after first checking whether any outlier should be renamed into that family"
            ),
        );
        AnalysisFinding {
            span: self.0[0].span,
            message: "type-family declarations are not covered by a section divider".to_owned(),
            help: guidance,
            replacement: None,
        }
    }
}

struct Section {
    divider: SectionDividerOccurrence,
    participants: SectionParticipantList,
}

fn participant_name(cx: &LateContext<'_>, item: &Item<'_>) -> Option<String> {
    if item.span.from_expansion() {
        return None;
    }
    match item.kind {
        ItemKind::Struct(..)
        | ItemKind::Enum(..)
        | ItemKind::Union(..)
        | ItemKind::TyAlias(..)
        | ItemKind::Trait(..)
        | ItemKind::TraitAlias(..) => Some(cx.tcx.item_name(item.owner_id.to_def_id()).to_string()),
        ItemKind::Impl(_) => {
            let self_type = cx.tcx.type_of(item.owner_id).instantiate_identity();
            let ty::Adt(definition, _) = self_type.kind() else {
                return None;
            };
            definition
                .did()
                .as_local()
                .map(|definition| cx.tcx.item_name(definition.to_def_id()).to_string())
        }
        _ => None,
    }
}

// -----------------------------------------------------------------------------
// ContentParseResult: Parsed divider content
// -----------------------------------------------------------------------------

struct ContentParseResult {
    prefix: Option<String>,
    normalized: Option<String>,
    error: Option<String>,
}

fn parse_content(content: &str) -> ContentParseResult {
    let trimmed = content.trim();
    let (raw_prefix, raw_description) = trimmed
        .split_once(':')
        .map_or((trimmed, None), |(prefix, description)| {
            (prefix.trim(), Some(description.trim()))
        });
    let prefix = valid_pascal_case(raw_prefix).then(|| raw_prefix.to_owned());
    let mut normalized_description = raw_description.map(ToOwned::to_owned);
    if let Some(description) = &mut normalized_description
        && let Some(first) = description.chars().next()
        && first.is_ascii_lowercase()
    {
        description.replace_range(..first.len_utf8(), &first.to_ascii_uppercase().to_string());
    }
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
    } else if raw_description.is_some_and(|description| {
        description
            .chars()
            .find(|character| character.is_alphabetic())
            .is_some_and(|character| !character.is_uppercase())
    }) {
        Some("section divider description must start in sentence case".to_owned())
    } else if normalized.as_deref() != Some(content) {
        Some("section divider content has non-canonical spacing".to_owned())
    } else {
        None
    };

    ContentParseResult {
        prefix,
        normalized,
        error,
    }
}

fn valid_pascal_case(value: &str) -> bool {
    value.chars().next().is_some_and(char::is_uppercase) && value.chars().all(char::is_alphanumeric)
}

fn longest_word_prefix(names: &[&str]) -> Option<String> {
    let first = *names.first()?;
    let mut boundaries = first
        .char_indices()
        .filter_map(|(index, character)| (index > 0 && character.is_uppercase()).then_some(index))
        .collect::<Vec<_>>();
    boundaries.push(first.len());
    boundaries.into_iter().rev().find_map(|end| {
        let prefix = &first[..end];
        (valid_pascal_case(prefix)
            && names.iter().all(|name| {
                name.strip_prefix(prefix).is_some_and(|rest| {
                    rest.is_empty() || rest.chars().next().is_some_and(char::is_uppercase)
                })
            }))
        .then(|| prefix.to_owned())
    })
}

// -----------------------------------------------------------------------------
// Template: Configured template matching and rendering
// -----------------------------------------------------------------------------

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
        if source.matches(CONTENT_PLACEHOLDER).count() != 1 {
            return Err("template must contain exactly one `{content}` placeholder".to_owned());
        }
        let lines = source
            .split('\n')
            .map(|line| {
                if !line.starts_with("//") || line.starts_with("///") || line.starts_with("//!") {
                    return Err("every template line must be a normal `//` comment".to_owned());
                }
                if line.replace(CONTENT_PLACEHOLDER, "").chars().count() > max_line_length {
                    return Err("a static template line exceeds max_line_length".to_owned());
                }
                let (before, after) = line.split_once(CONTENT_PLACEHOLDER).map_or_else(
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
        self.source.replace(CONTENT_PLACEHOLDER, content)
    }

    fn find_matches(&self, source: &str) -> Vec<TemplateMatch> {
        let lines = source_lines(source);
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

fn source_lines(source: &str) -> Vec<TemplateSourceLine<'_>> {
    let mut start = 0;
    source
        .split_inclusive('\n')
        .map(|line| {
            let body = line.strip_suffix('\n').unwrap_or(line);
            let indentation_end = body
                .char_indices()
                .find_map(|(index, character)| (!matches!(character, ' ' | '\t')).then_some(index))
                .unwrap_or(body.len());
            let result = TemplateSourceLine {
                start,
                indentation: &body[..indentation_end],
                content: &body[indentation_end..],
            };
            start += line.len();
            result
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{Template, longest_word_prefix, parse_content};

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
    fn finds_longest_pascal_case_word_prefix() {
        assert_eq!(
            longest_word_prefix(&["Candidate", "CandidateBindingUse"]),
            Some("Candidate".to_owned())
        );
        assert_eq!(
            longest_word_prefix(&["MyThing", "MyOther"]),
            Some("My".to_owned())
        );
        assert_eq!(longest_word_prefix(&["MyThing", "Model"]), None);
    }

    #[test]
    fn normalizes_safe_content_errors() {
        let parsed = parse_content("Candidate : collected state");
        assert!(parsed.error.is_some());
        assert_eq!(
            parsed.normalized.as_deref(),
            Some("Candidate: Collected state")
        );
    }
}
