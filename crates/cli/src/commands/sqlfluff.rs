//! Shared `SQLFluff` policy and bundled backend integration.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::{env, fs};

use clap::ValueEnum;
use color_eyre::eyre::WrapErr as _;
use color_eyre::{Section as _, SectionExt as _};
use rlib_sqlx_model::protocol::{QueryDocument, QueryOrigin};
use semver::{Version, VersionReq};
use serde::Deserialize;

use super::error::{CommandResult, Failure};

// -----------------------------------------------------------------------------
// InternalEnvSqlfluff: Bundled backend location
// -----------------------------------------------------------------------------

/// Private environment variable locating the bundled SQL analysis backend.
const INTERNAL_ENV_SQLFLUFF: &str = "RLIB_INTERNAL_SQLFLUFF_PATH";

// -----------------------------------------------------------------------------
// SqlfluffConfigNames: Project policy discovery
// -----------------------------------------------------------------------------

/// `SQLFluff` configuration filenames recognized while walking project ancestors.
const SQLFLUFF_CONFIG_NAMES: &[&str] = &[
    "setup.cfg",
    "tox.ini",
    "pep8.ini",
    ".sqlfluff",
    "pyproject.toml",
];

// -----------------------------------------------------------------------------
// SqlDialect: Initialization policy
// -----------------------------------------------------------------------------

/// SQL dialects supported by project initialization.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum SqlDialect {
    /// `PostgreSQL` grammar and placeholders.
    Postgres,
    /// `MySQL` grammar and placeholders.
    Mysql,
    /// `SQLite` grammar and placeholders.
    Sqlite,
}

impl SqlDialect {
    /// Returns the `SQLFluff` dialect name.
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Postgres => "postgres",
            Self::Mysql => "mysql",
            Self::Sqlite => "sqlite",
        }
    }

    /// Returns the placeholder syntax used by this database.
    pub(super) const fn placeholder_style(self) -> &'static str {
        match self {
            Self::Postgres => "numeric_dollar",
            Self::Mysql | Self::Sqlite => "question_mark",
        }
    }
}

// -----------------------------------------------------------------------------
// SqlfluffFinding: External diagnostic contract
// -----------------------------------------------------------------------------

/// One finding returned by `SQLFluff`.
#[derive(Deserialize)]
pub(super) struct SqlfluffFinding {
    /// One-based line in the SQL input.
    start_line_no: usize,
    /// One-based column in the SQL input.
    start_line_pos: usize,
    /// Stable `SQLFluff` rule code.
    code: String,
    /// Human-readable rule description.
    description: String,
    /// Whether project policy marks this finding as non-enforcing.
    #[serde(default = "is_missing_warning_enforcing", rename = "warning")]
    is_warning: bool,
}

impl SqlfluffFinding {
    /// Returns the one-based SQL line containing the finding.
    pub(super) const fn line(&self) -> usize {
        self.start_line_no
    }

    /// Returns the one-based SQL column containing the finding.
    pub(super) const fn column(&self) -> usize {
        self.start_line_pos
    }

    /// Returns the stable backend rule code.
    pub(super) fn code(&self) -> &str {
        &self.code
    }

    /// Returns the human-readable finding description.
    pub(super) fn description(&self) -> &str {
        &self.description
    }

    /// Returns whether project policy treats this finding as a warning.
    pub(super) const fn is_warning(&self) -> bool {
        self.is_warning
    }
}

/// Treats an absent `SQLFluff` warning flag as an enforcing finding.
const fn is_missing_warning_enforcing() -> bool {
    false
}

// -----------------------------------------------------------------------------
// SqlfluffRecord: External document contract
// -----------------------------------------------------------------------------

/// Parsed `SQLFluff` output for one linted SQL input.
#[derive(Deserialize)]
struct SqlfluffRecord {
    /// Findings reported for one input file.
    violations: Vec<SqlfluffFinding>,
}

// -----------------------------------------------------------------------------
// QueryOriginExt: Authored source identity
// -----------------------------------------------------------------------------

/// Local source-path behavior for compiler query provenance.
pub(super) trait QueryOriginExt {
    /// Returns the authored file owning this query.
    fn authored_path(&self) -> &Path;
}

impl QueryOriginExt for QueryOrigin {
    fn authored_path(&self) -> &Path {
        match self {
            Self::Inline { rust_path, .. } => rust_path,
            Self::File { sql_path } => sql_path,
        }
    }
}

/// Extracts a semantic version from `SQLFluff --version` output.
fn parse_version(stdout: &str) -> CommandResult<Version> {
    let version_token = stdout
        .split_whitespace()
        .map(|token| token.trim_start_matches('v'))
        .find(|token| {
            token.contains('.')
                && token
                    .chars()
                    .all(|character| character.is_ascii_digit() || character == '.')
        })
        .ok_or_else(|| Failure::tool(format!("unrecognized SQLFluff version output: {stdout}")))?;
    Version::parse(version_token).map_err(|error| {
        Failure::tool(format!(
            "invalid SQLFluff version `{version_token}`: {error}"
        ))
    })
}

// -----------------------------------------------------------------------------
// BundledSqlfluff: Backend process owner
// -----------------------------------------------------------------------------

/// The SQL backend bundled with the rlib distribution.
pub(super) struct BundledSqlfluff {
    /// Installed backend executable.
    executable: PathBuf,
}

impl BundledSqlfluff {
    /// Lints one compiler-extracted query.
    pub(super) fn lint(&self, document: &QueryDocument) -> CommandResult<Vec<SqlfluffFinding>> {
        let mut command = Command::new(&self.executable);
        command.args([
            "lint",
            "--format",
            "json",
            "--nocolor",
            "--disable-progress-bar",
        ]);
        match &document.origin {
            QueryOrigin::File { sql_path } => {
                command.arg(sql_path);
            }
            QueryOrigin::Inline { rust_path, .. } => {
                command
                    .args(["--stdin-filename"])
                    .arg(rust_path.with_extension("rs.sql"))
                    .arg("-")
                    .stdin(Stdio::piped());
            }
        }
        command.stdout(Stdio::piped()).stderr(Stdio::piped());

        let mut child = command.spawn().wrap_err_with(|| {
            format!(
                "could not start SQLFluff for `{}`",
                document.origin.authored_path().display()
            )
        })?;
        if matches!(document.origin, QueryOrigin::Inline { .. }) {
            child
                .stdin
                .take()
                .expect("piped stdin exists")
                .write_all(document.text.as_bytes())?;
        }
        let output = child.wait_with_output().wrap_err_with(|| {
            format!(
                "could not collect SQLFluff output for `{}`",
                document.origin.authored_path().display()
            )
        })?;

        // Statuses beyond clean or findings mean the backend could not analyze the input.
        if !matches!(output.status.code(), Some(0 | 1)) {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            return Err(Failure::tool(format!(
                "SQLFluff could not analyze `{}`",
                document.origin.authored_path().display()
            ))
            .section(stderr.header("SQLFluff stderr:"))
            .suggestion("Run SQLFluff directly against this query to inspect its configuration."));
        }
        let records: Vec<SqlfluffRecord> = serde_json::from_slice(&output.stdout)
            .map_err(|error| Failure::tool(format!("SQLFluff returned invalid JSON: {error}")))?;
        Ok(records
            .into_iter()
            .flat_map(|record| record.violations)
            .collect())
    }

    /// Verifies that the backend implements the expected JSON protocol.
    fn verify_version(&self) -> CommandResult {
        let output = Command::new(&self.executable)
            .arg("--version")
            .output()
            .map_err(|error| Failure::tool(format!("bundled SQL backend failed: {error}")))?;

        // A failing version probe cannot establish protocol compatibility.
        if !output.status.success() {
            return Err(
                Failure::tool("could not determine the bundled SQLFluff version")
                    .suggestion("Reinstall cargo-rlib to restore its bundled SQL tooling."),
            );
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let version = parse_version(&stdout)?;
        let supported = VersionReq::parse(">=4,<5").expect("fixed version requirement is valid");
        if supported.matches(&version) {
            Ok(())
        } else {
            Err(
                Failure::tool(format!("SQLFluff {version} is unsupported; expected 4.x"))
                    .suggestion("Update cargo-rlib so its bundled SQLFluff version matches."),
            )
        }
    }

    /// Resolves and validates the bundled backend.
    pub(super) fn load() -> CommandResult<Self> {
        let executable = env::var_os(INTERNAL_ENV_SQLFLUFF).ok_or_else(|| {
            Failure::tool("the SQL analysis backend is unavailable")
                .suggestion("Reinstall cargo-rlib to restore its bundled SQL tooling.")
        })?;
        let backend = Self {
            executable: executable.into(),
        };
        backend.verify_version()?;
        Ok(backend)
    }
}

/// Returns whether a candidate file contains a `SQLFluff` configuration section.
fn is_sqlfluff_config(path: &Path) -> bool {
    // Unreadable candidates do not establish project policy.
    let Ok(source) = fs::read_to_string(path) else {
        return false;
    };
    if path
        .file_name()
        .is_some_and(|name| name == "pyproject.toml")
    {
        source.contains("[tool.sqlfluff")
    } else {
        source.contains("[sqlfluff")
    }
}

/// Finds applicable project-owned `SQLFluff` configuration files.
fn project_config_files<'a>(
    origin: &'a Path,
    workspace: &'a Path,
) -> impl Iterator<Item = PathBuf> + 'a {
    origin
        .ancestors()
        .take_while(move |ancestor| ancestor.starts_with(workspace))
        .flat_map(|directory| {
            SQLFLUFF_CONFIG_NAMES
                .iter()
                .map(move |name| directory.join(name))
        })
        .filter(|path| path.is_file() && is_sqlfluff_config(path))
}

// -----------------------------------------------------------------------------
// PolicyScope: Applicable project policy
// -----------------------------------------------------------------------------

/// Project-policy search rooted at one authored source location.
pub(super) struct PolicyScope<'a> {
    /// Directory containing the authored source.
    origin: &'a Path,
    /// Workspace boundary beyond which policy is not inherited.
    workspace: &'a Path,
}

impl<'a> PolicyScope<'a> {
    /// Binds a source directory to its workspace policy boundary.
    pub(super) const fn new(origin: &'a Path, workspace: &'a Path) -> Self {
        Self { origin, workspace }
    }

    /// Returns whether project policy opts this source into SQL analysis.
    pub(super) fn is_configured(&self) -> bool {
        project_config_files(self.origin, self.workspace)
            .next()
            .is_some()
    }

    /// Returns whether applicable project policy declares a database dialect.
    pub(super) fn has_dialect(&self) -> bool {
        project_config_files(self.origin, self.workspace).any(|path| {
            fs::read_to_string(path).is_ok_and(|source| {
                source.lines().any(|line| {
                    line.split_once('=').is_some_and(|(key, value)| {
                        key.trim() == "dialect" && !value.trim().trim_matches('"').is_empty()
                    })
                })
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use semver::Version;

    use super::{SqlDialect, parse_version};

    #[test]
    fn selects_placeholders_from_database_syntax() {
        assert_eq!(SqlDialect::Postgres.placeholder_style(), "numeric_dollar");
        assert_eq!(SqlDialect::Mysql.placeholder_style(), "question_mark");
        assert_eq!(SqlDialect::Sqlite.placeholder_style(), "question_mark");
    }

    #[test]
    fn parses_supported_backend_version_output() {
        let version = parse_version("sqlfluff, version 4.3.0")
            .expect("supported backend output should parse");
        assert_eq!(version, Version::new(4, 3, 0));
    }
}
