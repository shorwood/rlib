//! Cargo-compatible rendering for external SQL findings.

use std::fs;
use std::path::PathBuf;

use cargo_metadata::{Metadata, Package, Target};
use color_eyre::eyre::WrapErr as _;
use rlib_sqlx_model::protocol::{
    ByteRange, CompilationContext, QueryDocument, QueryOrigin, SourceSegment,
};

use super::manifest::ExtractedQuery;
use crate::commands::error::{CommandResult, Failure};
use crate::commands::sqlfluff::SqlfluffFinding;

// -----------------------------------------------------------------------------
// Emitter: Diagnostic presentation boundary
// -----------------------------------------------------------------------------

/// Emits external findings in the Cargo format selected by the invocation.
pub(super) struct Emitter<'a> {
    /// Diagnostic encoding requested through Cargo arguments.
    format: Format,
    /// Cargo identities required by machine-readable diagnostics.
    metadata: &'a Metadata,
}

impl<'a> Emitter<'a> {
    /// Selects diagnostic presentation from the original Cargo invocation.
    pub(super) fn new(arguments: &[String], metadata: &'a Metadata) -> Self {
        Self {
            format: Format::from_arguments(arguments),
            metadata,
        }
    }

    /// Writes one fully mapped finding using the selected Cargo encoding.
    fn render(
        &self,
        compilation: &CompilationContext,
        location: &DiagnosticLocation,
        finding: &SqlfluffFinding,
        severity: Severity,
    ) -> CommandResult {
        let SourceCoordinate { line, column } = location.coordinate;
        let level = severity.as_str();
        let code = format!("rlib::sql::{}", finding.code());
        if matches!(self.format, Format::Json) {
            let CargoDiagnosticContext { package, target } =
                CargoDiagnosticContext::resolve(self.metadata, compilation)?;
            let source = fs::read_to_string(&location.path).wrap_err_with(|| {
                format!(
                    "could not read diagnostic source `{}`",
                    location.path.display()
                )
            })?;
            let line_text = source.lines().nth(line.saturating_sub(1)).unwrap_or("");
            let byte_end = location.byte_offset.saturating_add(1).min(source.len());
            let rendered = format!(
                "{}:{line}:{column}: {level}[{code}]: {}\n",
                location.path.display(),
                finding.description()
            );
            let message = serde_json::json!({
                "reason": "compiler-message",
                "package_id": package.id,
                "manifest_path": package.manifest_path,
                "target": target,
                "message": {
                    "$message_type": "diagnostic",
                    "message": finding.description(),
                    "code": { "code": code, "explanation": null },
                    "level": level,
                    "spans": [{
                        "file_name": location.path,
                        "byte_start": location.byte_offset,
                        "byte_end": byte_end,
                        "line_start": line,
                        "line_end": line,
                        "column_start": column,
                        "column_end": column.saturating_add(1),
                        "is_primary": true,
                        "text": [{
                            "text": line_text,
                            "highlight_start": column,
                            "highlight_end": column.saturating_add(1)
                        }],
                        "label": null,
                        "suggested_replacement": null,
                        "suggestion_applicability": null,
                        "expansion": null
                    }],
                    "children": [],
                    "rendered": rendered
                }
            });
            println!("{message}");
        } else {
            eprintln!(
                "{}:{line}:{column}: {level}[{code}]: {}",
                location.path.display(),
                finding.description()
            );
        }
        Ok(())
    }

    /// Maps and emits one backend finding at its authored source location.
    pub(super) fn emit(
        &self,
        query: &ExtractedQuery,
        finding: &SqlfluffFinding,
    ) -> CommandResult<Severity> {
        let location = DiagnosticLocation::for_finding(&query.document, finding)?;
        let severity = Severity::for_finding(finding, &query.compilation);
        self.render(&query.compilation, &location, finding, severity)?;
        Ok(severity)
    }
}

// -----------------------------------------------------------------------------
// Format: Cargo diagnostic encoding
// -----------------------------------------------------------------------------

/// Diagnostic encoding selected through Cargo's message-format option.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Format {
    /// Standard rustc-style one-line diagnostic.
    Human,
    /// Cargo's short one-line diagnostic.
    Short,
    /// Cargo compiler-message JSON stream.
    Json,
}

impl Format {
    /// Resolves the last Cargo message-format option before the rustc delimiter.
    fn from_arguments(arguments: &[String]) -> Self {
        let mut format = Self::Human;
        let mut arguments = arguments
            .iter()
            .take_while(|argument| argument.as_str() != "--");
        while let Some(argument) = arguments.next() {
            let value = argument.strip_prefix("--message-format=").or_else(|| {
                (argument == "--message-format")
                    .then(|| arguments.next())
                    .flatten()
                    .map(String::as_str)
            });
            if let Some(value) = value {
                format = if value.starts_with("json") {
                    Self::Json
                } else if value == "short" {
                    Self::Short
                } else {
                    Self::Human
                };
            }
        }
        format
    }
}

// -----------------------------------------------------------------------------
// Severity: Finding enforcement policy
// -----------------------------------------------------------------------------

/// Effective diagnostic severity after applying rustc warning policy.
#[derive(Clone, Copy)]
pub(super) enum Severity {
    /// Non-enforcing project warning.
    Warning,
    /// Enforcing error or denied warning.
    Error,
}

impl Severity {
    /// Resolves backend and compiler warning policy into one severity.
    const fn for_finding(finding: &SqlfluffFinding, compilation: &CompilationContext) -> Self {
        if finding.is_warning() && !compilation.has_denied_warnings {
            Self::Warning
        } else {
            Self::Error
        }
    }

    /// Returns whether this finding makes the command fail.
    pub(super) const fn is_enforcing(self) -> bool {
        matches!(self, Self::Error)
    }

    /// Returns the rustc-compatible severity name.
    const fn as_str(self) -> &'static str {
        match self {
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

// -----------------------------------------------------------------------------
// SourceCoordinate: Typed authored position
// -----------------------------------------------------------------------------

/// One-based position in an authored source file.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceCoordinate {
    /// One-based source line.
    line: usize,
    /// One-based byte column.
    column: usize,
}

impl SourceCoordinate {
    /// Converts a byte offset to a one-based authored source position.
    fn at_offset(source: &str, offset: usize) -> Self {
        let before = &source[..offset.min(source.len())];
        let line = before.bytes().filter(|byte| *byte == b'\n').count() + 1;
        let column = before
            .rsplit_once('\n')
            .map_or(before.len(), |(_, tail)| tail.len())
            + 1;
        Self { line, column }
    }
}

// -----------------------------------------------------------------------------
// DiagnosticLocation: Authored finding location
// -----------------------------------------------------------------------------

/// Authored source location corresponding to an external SQL finding.
struct DiagnosticLocation {
    /// Authored SQL or Rust source file.
    path: PathBuf,
    /// One-based position in the authored source file.
    coordinate: SourceCoordinate,
    /// Zero-based byte offset in the authored source file.
    byte_offset: usize,
}

impl DiagnosticLocation {
    /// Maps one SQL finding to its authored SQL or Rust source location.
    fn for_finding(document: &QueryDocument, finding: &SqlfluffFinding) -> CommandResult<Self> {
        let sql_coordinate = SourceCoordinate {
            line: finding.line(),
            column: finding.column(),
        };
        match &document.origin {
            QueryOrigin::File { sql_path } => Ok(Self {
                path: sql_path.clone(),
                coordinate: sql_coordinate,
                byte_offset: sql_offset(&document.text, sql_coordinate),
            }),
            QueryOrigin::Inline {
                rust_path,
                literal_span,
                source_segments,
            } => {
                let sql = sql_offset(&document.text, sql_coordinate);
                let rust = mapped_rust_offset(sql, literal_span, source_segments)?;
                let source = fs::read_to_string(rust_path).wrap_err_with(|| {
                    format!("could not read query source `{}`", rust_path.display())
                })?;
                Ok(Self {
                    path: rust_path.clone(),
                    coordinate: SourceCoordinate::at_offset(&source, rust),
                    byte_offset: rust,
                })
            }
        }
    }
}

// -----------------------------------------------------------------------------
// CargoDiagnosticContext: Machine diagnostic identity
// -----------------------------------------------------------------------------

/// Cargo metadata resolved for one machine-readable diagnostic.
struct CargoDiagnosticContext<'a> {
    /// Cargo package owning the diagnostic.
    package: &'a Package,
    /// Cargo target compiled when the query was recovered.
    target: &'a Target,
}

impl<'a> CargoDiagnosticContext<'a> {
    /// Resolves the Cargo package and target that produced one extracted query.
    fn resolve(metadata: &'a Metadata, compilation: &CompilationContext) -> CommandResult<Self> {
        let package = metadata
            .packages
            .iter()
            .find(|package| {
                package
                    .manifest_path
                    .as_std_path()
                    .canonicalize()
                    .is_ok_and(|path| path == compilation.package_manifest)
            })
            .ok_or_else(|| {
                Failure::tool(format!(
                    "query compilation package `{}` is not in Cargo metadata",
                    compilation.package_manifest.display()
                ))
            })?;
        let mut targets = package
            .targets
            .iter()
            .filter(|target| target.name.replace('-', "_") == compilation.crate_name);
        let target = targets.next().ok_or_else(|| {
            Failure::tool(format!(
                "query compilation target `{}` is not in Cargo metadata",
                compilation.crate_name
            ))
        })?;

        // Ambiguous target identity cannot produce a trustworthy Cargo envelope.
        if targets.next().is_some() {
            return Err(Failure::tool(format!(
                "query compilation target `{}` is ambiguous in Cargo metadata",
                compilation.crate_name
            )));
        }
        Ok(Self { package, target })
    }
}

/// Converts a one-based SQL position to a byte offset in the extracted input.
fn sql_offset(source: &str, coordinate: SourceCoordinate) -> usize {
    let line_start = source
        .split_inclusive('\n')
        .take(coordinate.line.saturating_sub(1))
        .map(str::len)
        .sum::<usize>();
    line_start
        .saturating_add(coordinate.column.saturating_sub(1))
        .min(source.len())
}

/// Converts a protocol byte offset to the host platform's source offset type.
fn protocol_offset(offset: u32) -> CommandResult<usize> {
    usize::try_from(offset)
        .map_err(|error| Failure::tool(format!("query source offset is too large: {error}")))
}

/// Maps an extracted SQL offset back to its Rust string-literal source offset.
fn mapped_rust_offset(
    offset: usize,
    fallback: &ByteRange,
    mappings: &[SourceSegment],
) -> CommandResult<usize> {
    for mapping in mappings {
        let sql_start = protocol_offset(mapping.sql.start)?;
        let sql_end = protocol_offset(mapping.sql.end)?;
        let range = sql_start..sql_end;

        // The first containing segment owns this decoded SQL byte.
        if range.contains(&offset) || (range.is_empty() && range.start == offset) {
            let rust_start = protocol_offset(mapping.rust.start)?;
            let sql_length = mapping.sql.end.saturating_sub(mapping.sql.start);
            let rust_length = mapping.rust.end.saturating_sub(mapping.rust.start);
            return Ok(if sql_length == rust_length {
                rust_start.saturating_add(offset.saturating_sub(sql_start))
            } else {
                rust_start
            });
        }
    }
    protocol_offset(fallback.start)
}

#[cfg(test)]
mod tests {
    use rlib_sqlx_model::protocol::{ByteRange, SourceSegment};

    use super::{Format, SourceCoordinate, mapped_rust_offset};

    #[test]
    fn maps_equal_length_decoded_segments() {
        let mappings = [SourceSegment {
            sql: ByteRange { start: 0, end: 6 },
            rust: ByteRange { start: 10, end: 16 },
        }];
        assert!(matches!(
            mapped_rust_offset(4, &ByteRange { start: 8, end: 18 }, &mappings),
            Ok(14)
        ));
    }

    #[test]
    fn computes_one_based_source_coordinates() {
        assert_eq!(
            SourceCoordinate::at_offset("one\ntwo", 5),
            SourceCoordinate { line: 2, column: 2 }
        );
    }

    #[test]
    fn recognizes_cargo_json_message_formats() {
        let arguments = ["--message-format=json-diagnostic-rendered-ansi".to_owned()];
        assert_eq!(Format::from_arguments(&arguments), Format::Json);
    }
}
