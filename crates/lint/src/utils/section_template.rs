use crate::utils::{identifier_case, prose_case};

// -----------------------------------------------------------------------------
// ParsedContent: Parsed divider content
// -----------------------------------------------------------------------------

/// Canonical interpretation of the text captured by a divider template.
pub(super) struct ParsedContent {
    /// Valid `PascalCase` prefix, if the authored prefix can be used semantically.
    pub(super) prefix: Option<String>,
    /// Canonical content suitable for a safe replacement.
    pub(super) normalized: Option<String>,
    /// First syntax or casing failure found in the authored content.
    pub(super) error: Option<String>,
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
pub(super) struct TemplateMatch {
    /// Byte offset at which non-indentation template text begins.
    pub(super) start: usize,
    /// Byte offset immediately after the final template line.
    pub(super) end: usize,
    /// Text captured from the template placeholder.
    pub(super) content: String,
    /// Whitespace shared by every matched template line.
    pub(super) indentation: String,
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
pub(super) struct Template {
    /// Original template source retained for exact rendering.
    source: String,
    /// Parsed static fragments for each template line.
    lines: Vec<TemplateLine>,
}

impl Template {
    /// Validates and parses a configured divider template.
    pub(super) fn parse(source: &str, max_line_length: usize) -> Result<Self, String> {
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
    pub(super) fn render(&self, content: &str) -> String {
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
            let Ok(value) = Self::line_content(template, line) else {
                return None;
            };
            let Some(value) = value else {
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
    pub(super) fn find_matches(&self, source: &str) -> Vec<TemplateMatch> {
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
