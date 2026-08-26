//! `SQLx` native-lint and `SQLFluff` companion workflow.

use std::collections::BTreeMap;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{Error as IoError, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use cargo_metadata::{Metadata, MetadataCommand, Package, Target};
use dylint::opts::{Check, Dylint, LibrarySelection, Operation};
use rlib_sqlx_model::protocol::{
    ByteRange, CompilationContext, QUERY_MANIFEST_VERSION, QueryDocument, QueryManifest,
    QueryOrigin, SourceSegment,
};
use semver::{Version, VersionReq};
use serde::Deserialize;
use tempfile::{Builder, TempDir};

// -----------------------------------------------------------------------------
// ExtractionEnv: Compiler query handoff
// -----------------------------------------------------------------------------

/// Environment variable carrying the compiler extractor's output directory.
const EXTRACTION_ENV_MANIFEST: &str = "RLIB_SQLX_MANIFEST_DIR";

/// Environment variable carrying the canonical workspace trust boundary.
const EXTRACTION_ENV_WORKSPACE: &str = "RLIB_SQLX_WORKSPACE_ROOT";

// -----------------------------------------------------------------------------
// InternalEnv: Bundled runner handoff
// -----------------------------------------------------------------------------

/// Private environment variable carrying serialized Cargo arguments to the runner child.
const INTERNAL_ENV_CARGO_ARGUMENTS: &str = "RLIB_INTERNAL_CARGO_ARGUMENTS";

/// Private environment variable locating the bundled rlib lint library.
const INTERNAL_ENV_LINT_LIBRARY: &str = "RLIB_INTERNAL_LINT_LIBRARY_PATH";

/// Private environment variable locating the bundled SQL analysis backend.
const INTERNAL_ENV_SQLFLUFF: &str = "RLIB_INTERNAL_SQLFLUFF_PATH";

/// Dylint environment variable locating compiler drivers by toolchain.
const INTERNAL_ENV_DYLINT_DRIVERS: &str = "DYLINT_DRIVER_PATH";

/// Dylint environment variable carrying flags forwarded directly to rustc.
const INTERNAL_ENV_DYLINT_RUSTFLAGS: &str = "DYLINT_RUSTFLAGS";

/// Rust toolchain compatibility key used to select a Dylint driver.
const INTERNAL_ENV_TOOLCHAIN: &str = "RUSTUP_TOOLCHAIN";

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
// CommandErrorStatus: Stable process status
// -----------------------------------------------------------------------------

/// Process status categories exposed by the companion command.
#[derive(Clone, Copy, Debug)]
enum CommandErrorStatus {
    /// Enforcing external SQL findings were reported.
    Findings = 1,
    /// Required tooling, configuration, or command arguments were invalid.
    Tool = 2,
    /// Compiler-backed native analysis failed.
    Compiler = 101,
}

// -----------------------------------------------------------------------------
// CommandError: Stable process failure
// -----------------------------------------------------------------------------

/// Failure from a public companion command.
#[derive(Debug, derive_more::Display, derive_more::Error)]
#[display("{message}")]
pub struct CommandError {
    /// Human-readable failure context.
    message: String,
    /// Stable process status category.
    status: CommandErrorStatus,
}

impl CommandError {
    /// Builds a required-tool, configuration, or protocol failure.
    fn tool(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            status: CommandErrorStatus::Tool,
        }
    }

    /// Builds a compiler-analysis failure.
    fn compiler(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            status: CommandErrorStatus::Compiler,
        }
    }

    /// Builds an enforcing-`SQLFluff`-findings failure.
    fn findings() -> Self {
        Self {
            message: "SQLFluff found enforcing violations".to_owned(),
            status: CommandErrorStatus::Findings,
        }
    }

    /// Returns the stable process status for this failure category.
    pub(crate) const fn exit_code(&self) -> u8 {
        self.status as u8
    }
}

impl From<IoError> for CommandError {
    fn from(error: IoError) -> Self {
        Self::tool(error.to_string())
    }
}

// -----------------------------------------------------------------------------
// SqlxInit: Companion initialization operation
// -----------------------------------------------------------------------------

/// One request to create project-owned `SQLFluff` policy.
pub struct SqlxInit {
    /// Database SQL dialect.
    dialect: String,
    /// Optional directory receiving `.sqlfluff`.
    root: Option<PathBuf>,
}

impl SqlxInit {
    /// Creates an initialization request from validated CLI values.
    pub fn new(dialect: &str, root: Option<PathBuf>) -> Self {
        Self {
            dialect: dialect.to_owned(),
            root,
        }
    }

    /// Creates policy without overwriting an existing configuration.
    pub fn run(self) -> Result<(), CommandError> {
        let root = self.root.map_or_else(workspace_root, Ok)?;
        fs::create_dir_all(&root)?;

        // Existing project policy always owns initialization.
        if project_config_files(&root, &root).next().is_some() {
            return Err(CommandError::tool(format!(
                "SQLFluff configuration already exists at `{}`",
                root.display()
            )));
        }
        let path = root.join(".sqlfluff");
        let parameter_style = placeholder_style(&self.dialect);
        let contents = format!(
            "[sqlfluff]\ndialect = {}\ntemplater = placeholder\n\n[sqlfluff:templater:placeholder]\nparam_style = {parameter_style}\n",
            self.dialect
        );
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        file.write_all(contents.as_bytes())?;
        println!("created {}", path.display());
        Ok(())
    }
}

// -----------------------------------------------------------------------------
// RlibCommand: Complete analysis operation
// -----------------------------------------------------------------------------

/// One complete native and configured-adapter analysis operation.
pub struct RlibCommand {
    /// Arguments interpreted by Cargo for package and target selection.
    cargo_arguments: Vec<String>,
    /// Arguments forwarded directly to rustc after the public delimiter.
    rustc_arguments: Vec<String>,
    /// Whether machine-applicable compiler suggestions should be applied first.
    fix_mode: FixMode,
    /// Cargo-compatible diagnostic rendering requested by the invocation.
    output_format: OutputFormat,
}

impl RlibCommand {
    /// Creates an analysis operation from arguments already separated by the CLI parser.
    pub fn new(
        cargo_arguments: Vec<String>,
        rustc_arguments: Vec<String>,
        fix_mode: FixMode,
    ) -> Self {
        let output_format = OutputFormat::from_arguments(&cargo_arguments);
        Self {
            cargo_arguments,
            rustc_arguments,
            fix_mode,
            output_format,
        }
    }

    /// Runs native checks, configured SQL analysis, and optional native fixes.
    pub fn check(self) -> Result<(), CommandError> {
        // Run compiler analysis and select only queries with applicable SQL policy.
        let metadata = workspace_metadata()?;
        let root = metadata
            .workspace_root
            .clone()
            .into_std_path_buf()
            .canonicalize()?;
        if matches!(self.fix_mode, FixMode::Fix) {
            run_compiler(
                &root,
                &self.cargo_arguments,
                &self.rustc_arguments,
                None,
                FixMode::Fix,
            )?;
        }

        // Recover authored queries from the final non-mutating compiler pass.
        let temporary_root = root.join("target/rlib/sqlx");
        fs::create_dir_all(&temporary_root)?;
        let output = Builder::new().prefix("run-").tempdir_in(&temporary_root)?;
        run_compiler(
            &root,
            &self.cargo_arguments,
            &self.rustc_arguments,
            Some(output.path()),
            FixMode::Check,
        )?;
        let documents = load_manifests(output.path(), &root)?;

        // Only authored SQL policy opts a recovered query into external analysis.
        let configured = documents
            .iter()
            .filter(|document| {
                let origin = origin_path(&document.query.origin);
                let config_origin = origin.parent().unwrap_or(&root);
                project_config_files(config_origin, &root).next().is_some()
            })
            .collect::<Vec<_>>();

        // SQL policy is explicitly enabled by an applicable project configuration.
        if configured.is_empty() {
            return Ok(());
        }
        sqlfluff_version()?;
        for document in &configured {
            let origin = origin_path(&document.query.origin);
            let config_origin = origin.parent().unwrap_or(&root);

            // An opted-in SQL document must have enough policy to parse its dialect.
            if !has_project_dialect(config_origin, &root) {
                return Err(CommandError::tool(format!(
                    "SQLFluff configuration applying to `{}` does not declare a dialect",
                    origin.display()
                )));
            }
        }
        let mut enforcing = false;
        for document in configured {
            for violation in document.lint()? {
                let location = DiagnosticLocation::for_violation(&document.query, &violation)?;
                let is_warning = violation.is_warning && !document.compilation.has_denied_warnings;
                let level = if is_warning { "warning" } else { "error" };
                render_violation(
                    self.output_format,
                    &metadata,
                    &document.compilation,
                    &location,
                    &violation,
                    level,
                )?;
                enforcing |= !is_warning;
            }
        }
        if enforcing {
            Err(CommandError::findings())
        } else {
            Ok(())
        }
    }
}

// -----------------------------------------------------------------------------
// OutputFormat: Cargo-compatible diagnostic rendering
// -----------------------------------------------------------------------------

/// Diagnostic encoding selected through Cargo's message-format option.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OutputFormat {
    /// Standard rustc-style one-line diagnostic.
    Human,
    /// Cargo's short one-line diagnostic.
    Short,
    /// Cargo compiler-message JSON stream.
    Json,
}

impl OutputFormat {
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
// Fix: Compiler mutation policy
// -----------------------------------------------------------------------------

/// Parsed presence of the public `--fix` option.
#[derive(Clone, Copy)]
pub struct FixRequest {
    /// Whether the invocation requested compiler mutation.
    is_requested: bool,
}

impl From<bool> for FixRequest {
    /// Preserves the parsed option state behind a policy-specific type.
    fn from(is_requested: bool) -> Self {
        Self { is_requested }
    }
}

/// Whether the private compiler run checks source or applies native suggestions.
#[derive(Clone, Copy)]
pub enum FixMode {
    /// Analyze without changing authored source.
    Check,
    /// Apply machine-applicable compiler suggestions.
    Fix,
}

impl FixMode {
    /// Selects compiler mutation policy from a parsed request.
    pub const fn for_request(request: FixRequest) -> Self {
        if request.is_requested {
            Self::Fix
        } else {
            Self::Check
        }
    }
}

// -----------------------------------------------------------------------------
// CompilerDriverAlias: Per-invocation Cargo freshness identity
// -----------------------------------------------------------------------------

/// Temporary driver path that makes Cargo rerun workspace crates for extraction.
struct CompilerDriverAlias {
    /// Directory retaining the copied driver for the child process lifetime.
    directory: TempDir,
}

impl CompilerDriverAlias {
    /// Copies the bundled driver behind a unique path while preserving its toolchain layout.
    fn create(temporary_root: &Path) -> Result<Self, CommandError> {
        let drivers = env::var_os(INTERNAL_ENV_DYLINT_DRIVERS).ok_or_else(|| {
            CommandError::tool("the compiler driver is not bundled with this command")
        })?;
        let toolchain = env::var_os(INTERNAL_ENV_TOOLCHAIN).ok_or_else(|| {
            CommandError::tool("the compiler toolchain identity is not bundled with this command")
        })?;
        let executable = format!("dylint-driver{}", env::consts::EXE_SUFFIX);
        let source = PathBuf::from(drivers).join(&toolchain).join(&executable);

        // A missing source cannot be repaired safely by Dylint inside the installed package.
        if !source.is_file() {
            return Err(CommandError::tool(format!(
                "bundled compiler driver `{}` does not exist",
                source.display()
            )));
        }
        let directory = Builder::new()
            .prefix("driver-")
            .tempdir_in(temporary_root)?;
        let target_root = directory.path().join(toolchain);
        fs::create_dir_all(&target_root)?;
        fs::copy(source, target_root.join(executable))?;
        Ok(Self { directory })
    }

    /// Returns the driver collection root expected by Dylint.
    fn path(&self) -> &Path {
        self.directory.path()
    }
}

// -----------------------------------------------------------------------------
// SqlfluffViolation: External finding contract
// -----------------------------------------------------------------------------

/// One `SQLFluff` JSON finding.
#[derive(Deserialize)]
struct SqlfluffViolation {
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

// -----------------------------------------------------------------------------
// SqlfluffRecord: External document contract
// -----------------------------------------------------------------------------

/// Parsed `SQLFluff` output for one linted SQL input.
#[derive(Deserialize)]
struct SqlfluffRecord {
    /// Findings reported for one input file.
    violations: Vec<SqlfluffViolation>,
}

/// Invokes Dylint through its library API from the private runner child.
pub fn run_internal_dylint(fix_mode: FixMode) -> Result<(), CommandError> {
    let arguments = env::var(INTERNAL_ENV_CARGO_ARGUMENTS).map_err(|error| {
        CommandError::tool(format!("missing internal Cargo arguments: {error}"))
    })?;
    let arguments: Vec<String> = serde_json::from_str(&arguments).map_err(|error| {
        CommandError::tool(format!("invalid internal Cargo arguments: {error}"))
    })?;
    let lint_library = env::var(INTERNAL_ENV_LINT_LIBRARY).map_err(|error| {
        CommandError::tool(format!(
            "the rlib lint library is not bundled with this command: {error}"
        ))
    })?;
    let options = Dylint {
        operation: Operation::Check(Check {
            lib_sel: LibrarySelection {
                lib_paths: vec![lint_library],
                no_build: true,
                no_metadata: true,
                ..LibrarySelection::default()
            },
            fix: matches!(fix_mode, FixMode::Fix),
            args: arguments,
            ..Check::default()
        }),
        ..Dylint::default()
    };
    dylint::run(&options)
        .map_err(|error| CommandError::compiler(format!("compiler analysis failed: {error:#}")))
}

/// Treats an absent `SQLFluff` warning flag as an enforcing finding.
const fn is_missing_warning_enforcing() -> bool {
    false
}

/// Resolves Cargo workspace metadata for command routing and JSON diagnostics.
fn workspace_metadata() -> Result<Metadata, CommandError> {
    MetadataCommand::new()
        .no_deps()
        .exec()
        .map_err(|error| CommandError::tool(format!("could not read Cargo metadata: {error}")))
}

/// Resolves the canonical Cargo workspace containing the invocation.
fn workspace_root() -> Result<PathBuf, CommandError> {
    Ok(workspace_metadata()?.workspace_root.into_std_path_buf())
}

/// Returns the `SQLFluff` placeholder syntax for one validated database dialect.
fn placeholder_style(dialect: &str) -> &'static str {
    match dialect {
        "postgres" => "numeric_dollar",
        "mysql" | "sqlite" => "question_mark",
        _ => unreachable!("dialect is validated before placeholder selection"),
    }
}

/// Runs the private Dylint child with optional compiler-backed query extraction.
fn run_compiler(
    root: &Path,
    cargo_arguments: &[String],
    rustc_arguments: &[String],
    output: Option<&Path>,
    fix_mode: FixMode,
) -> Result<(), CommandError> {
    let temporary_root = root.join("target/rlib/sqlx");
    fs::create_dir_all(&temporary_root)?;
    let driver = CompilerDriverAlias::create(&temporary_root)?;
    let mut command = Command::new(env::current_exe()?);
    command.current_dir(root);
    command.arg("__run-dylint");
    command.env(INTERNAL_ENV_DYLINT_DRIVERS, driver.path());
    if matches!(fix_mode, FixMode::Fix) {
        command.arg("--fix");
    }
    command.env(
        INTERNAL_ENV_CARGO_ARGUMENTS,
        serde_json::to_string(cargo_arguments).map_err(|error| {
            CommandError::tool(format!("could not serialize Cargo arguments: {error}"))
        })?,
    );
    if !rustc_arguments.is_empty() {
        // Non-Unicode inherited flags cannot be combined into Dylint's textual protocol.
        let inherited = match env::var(INTERNAL_ENV_DYLINT_RUSTFLAGS) {
            Ok(flags) => flags,
            Err(env::VarError::NotPresent) => String::new(),
            // Lossy flag conversion could silently change the compiler policy.
            Err(error @ env::VarError::NotUnicode(_)) => {
                return Err(CommandError::tool(format!(
                    "inherited Dylint rustc flags are invalid: {error}"
                )));
            }
        };
        let rustc_arguments = rustc_arguments.join(" ");
        let flags = if inherited.is_empty() {
            rustc_arguments
        } else {
            format!("{inherited} {rustc_arguments}")
        };
        command.env(INTERNAL_ENV_DYLINT_RUSTFLAGS, flags);
    }
    if let Some(output) = output {
        command.env(EXTRACTION_ENV_MANIFEST, output);
        command.env(EXTRACTION_ENV_WORKSPACE, root);
    }
    let status = command.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(CommandError::compiler(format!(
            "SQLx compiler analysis failed with {status}"
        )))
    }
}

/// Creates a command for the SQL backend bundled with the rlib distribution.
fn sqlfluff_command() -> Result<Command, CommandError> {
    let path = env::var_os(INTERNAL_ENV_SQLFLUFF).ok_or_else(|| {
        CommandError::tool("the SQL analysis backend is not bundled with this command")
    })?;
    Ok(Command::new(path))
}

/// Verifies that the installed `SQLFluff` version implements the expected JSON protocol.
fn sqlfluff_version() -> Result<(), CommandError> {
    let output = sqlfluff_command()?
        .arg("--version")
        .output()
        .map_err(|error| CommandError::tool(format!("bundled SQL backend failed: {error}")))?;

    // A failing version probe cannot establish protocol compatibility.
    if !output.status.success() {
        return Err(CommandError::tool(
            "could not determine SQLFluff version".to_owned(),
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let version_token = stdout
        .split_whitespace()
        .map(|token| token.trim_start_matches('v'))
        .find(|token| {
            token.contains('.')
                && token
                    .chars()
                    .all(|character| character.is_ascii_digit() || character == '.')
        })
        .ok_or_else(|| {
            CommandError::tool(format!("unrecognized SQLFluff version output: {stdout}"))
        })?;
    let version = Version::parse(version_token).map_err(|error| {
        CommandError::tool(format!(
            "invalid SQLFluff version `{version_token}`: {error}"
        ))
    })?;
    let supported = VersionReq::parse(">=4,<5").expect("fixed version requirement is valid");
    if supported.matches(&version) {
        Ok(())
    } else {
        Err(CommandError::tool(format!(
            "SQLFluff {version} is unsupported; expected 4.x"
        )))
    }
}

/// Returns the authored file owning one query document.
fn origin_path(origin: &QueryOrigin) -> &Path {
    match origin {
        QueryOrigin::Inline { rust_path, .. } => rust_path,
        QueryOrigin::File { sql_path } => sql_path,
    }
}

// -----------------------------------------------------------------------------
// ExtractedDocument: Query and Cargo identity
// -----------------------------------------------------------------------------

/// One recovered query paired with the compilation that produced it.
struct ExtractedDocument {
    /// Recovered SQL document and authored provenance.
    query: QueryDocument,
    /// Cargo compilation identity used for machine diagnostics.
    compilation: CompilationContext,
}

impl ExtractedDocument {
    /// Invokes the bundled SQL analyzer for this compiler-extracted query.
    fn lint(&self) -> Result<Vec<SqlfluffViolation>, CommandError> {
        // Configure one backend process around the query's authored source identity.
        let mut command = sqlfluff_command()?;
        command.args([
            "lint",
            "--format",
            "json",
            "--nocolor",
            "--disable-progress-bar",
        ]);
        match &self.query.origin {
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

        // Execute the backend with either its authored file or extracted SQL input.
        let mut child = command.spawn()?;
        if matches!(self.query.origin, QueryOrigin::Inline { .. }) {
            child
                .stdin
                .take()
                .expect("piped stdin exists")
                .write_all(self.query.text.as_bytes())?;
        }
        let output = child.wait_with_output()?;

        // Statuses beyond clean or findings mean SQLFluff could not analyze the input.
        if !matches!(output.status.code(), Some(0 | 1)) {
            return Err(CommandError::tool(format!(
                "SQLFluff failed for `{}`: {}",
                origin_path(&self.query.origin).display(),
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        let records: Vec<SqlfluffRecord> =
            serde_json::from_slice(&output.stdout).map_err(|error| {
                CommandError::tool(format!("SQLFluff returned invalid JSON: {error}"))
            })?;
        Ok(records
            .into_iter()
            .flat_map(|record| record.violations)
            .collect())
    }
}

/// Loads, validates, and deterministically merges compiler query manifests.
fn load_manifests(
    directory: &Path,
    workspace: &Path,
) -> Result<Vec<ExtractedDocument>, CommandError> {
    let mut documents = BTreeMap::new();
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.extension().is_none_or(|extension| extension != "json") {
            continue;
        }
        let manifest: QueryManifest =
            serde_json::from_slice(&fs::read(&path)?).map_err(|error| {
                CommandError::tool(format!(
                    "invalid query manifest `{}`: {error}",
                    path.display()
                ))
            })?;

        // Protocol mismatches cannot be interpreted safely across tool versions.
        if manifest.version != QUERY_MANIFEST_VERSION {
            return Err(CommandError::tool(format!(
                "unsupported query manifest version {}",
                manifest.version
            )));
        }
        let manifest_workspace = manifest
            .workspace_root
            .canonicalize()
            .unwrap_or(manifest.workspace_root);

        // A manifest may describe only the workspace that requested extraction.
        if manifest_workspace != workspace {
            return Err(CommandError::tool(format!(
                "query manifest workspace `{}` does not match `{}`",
                manifest_workspace.display(),
                workspace.display()
            )));
        }
        for document in manifest.documents {
            let origin_path = origin_path(&document.origin);
            let canonical = origin_path
                .canonicalize()
                .unwrap_or_else(|_| origin_path.to_path_buf());

            // Source paths outside the workspace are not trusted diagnostic targets.
            if !canonical.starts_with(workspace) {
                return Err(CommandError::tool(format!(
                    "query source `{}` is outside workspace `{}`",
                    canonical.display(),
                    workspace.display()
                )));
            }
            let key = format!("{}\0{}", canonical.display(), document.text);
            documents.entry(key).or_insert_with(|| ExtractedDocument {
                query: document,
                compilation: manifest.compilation.clone(),
            });
        }
    }
    Ok(documents.into_values().collect())
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

/// Returns whether an applicable configuration declares a database dialect.
fn has_project_dialect(origin: &Path, workspace: &Path) -> bool {
    project_config_files(origin, workspace).any(|path| {
        fs::read_to_string(path).is_ok_and(|source| {
            source.lines().any(|line| {
                line.split_once('=').is_some_and(|(key, value)| {
                    key.trim() == "dialect" && !value.trim().trim_matches('"').is_empty()
                })
            })
        })
    })
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
    /// Maps one `SQLFluff` finding to its authored SQL or Rust source location.
    fn for_violation(
        document: &QueryDocument,
        violation: &SqlfluffViolation,
    ) -> Result<Self, CommandError> {
        let sql_coordinate = SourceCoordinate {
            line: violation.start_line_no,
            column: violation.start_line_pos,
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
                let source = fs::read_to_string(rust_path)?;
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
    fn resolve(
        metadata: &'a Metadata,
        compilation: &CompilationContext,
    ) -> Result<Self, CommandError> {
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
                CommandError::tool(format!(
                    "query compilation package `{}` is not in Cargo metadata",
                    compilation.package_manifest.display()
                ))
            })?;
        let mut targets = package
            .targets
            .iter()
            .filter(|target| target.name.replace('-', "_") == compilation.crate_name);
        let target = targets.next().ok_or_else(|| {
            CommandError::tool(format!(
                "query compilation target `{}` is not in Cargo metadata",
                compilation.crate_name
            ))
        })?;

        // Ambiguous target identity cannot produce a trustworthy Cargo diagnostic envelope.
        if targets.next().is_some() {
            return Err(CommandError::tool(format!(
                "query compilation target `{}` is ambiguous in Cargo metadata",
                compilation.crate_name
            )));
        }
        Ok(Self { package, target })
    }
}

/// Emits one SQL backend finding using the selected Cargo diagnostic encoding.
fn render_violation(
    format: OutputFormat,
    metadata: &Metadata,
    compilation: &CompilationContext,
    location: &DiagnosticLocation,
    violation: &SqlfluffViolation,
    level: &str,
) -> Result<(), CommandError> {
    let SourceCoordinate { line, column } = location.coordinate;
    let code = format!("rlib::sql::{}", violation.code);
    if matches!(format, OutputFormat::Json) {
        let CargoDiagnosticContext { package, target } =
            CargoDiagnosticContext::resolve(metadata, compilation)?;
        let source = fs::read_to_string(&location.path)?;
        let line_text = source.lines().nth(line.saturating_sub(1)).unwrap_or("");
        let byte_end = location.byte_offset.saturating_add(1).min(source.len());
        let rendered = format!(
            "{}:{line}:{column}: {level}[{code}]: {}\n",
            location.path.display(),
            violation.description
        );
        let message = serde_json::json!({
            "reason": "compiler-message",
            "package_id": package.id,
            "manifest_path": package.manifest_path,
            "target": target,
            "message": {
                "$message_type": "diagnostic",
                "message": violation.description,
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
            violation.description
        );
    }
    Ok(())
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
fn protocol_offset(offset: u32) -> Result<usize, CommandError> {
    usize::try_from(offset)
        .map_err(|error| CommandError::tool(format!("query source offset is too large: {error}")))
}

/// Maps an extracted SQL offset back to its Rust string-literal source offset.
fn mapped_rust_offset(
    offset: usize,
    fallback: &ByteRange,
    mappings: &[SourceSegment],
) -> Result<usize, CommandError> {
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

    use super::{
        FixMode, OutputFormat, RlibCommand, SourceCoordinate, mapped_rust_offset, placeholder_style,
    };

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
    fn selects_placeholders_from_database_syntax() {
        assert_eq!(placeholder_style("postgres"), "numeric_dollar");
        assert_eq!(placeholder_style("mysql"), "question_mark");
        assert_eq!(placeholder_style("sqlite"), "question_mark");
    }

    #[test]
    fn retains_separated_command_arguments() {
        let command = RlibCommand::new(
            vec!["--workspace".to_owned()],
            vec!["-D".to_owned(), "warnings".to_owned()],
            FixMode::Fix,
        );
        assert!(matches!(command.fix_mode, FixMode::Fix));
        assert_eq!(command.cargo_arguments, ["--workspace"]);
        assert_eq!(command.rustc_arguments, ["-D", "warnings"]);
    }

    #[test]
    fn recognizes_cargo_json_message_formats() {
        let command = RlibCommand::new(
            vec!["--message-format=json-diagnostic-rendered-ansi".to_owned()],
            Vec::new(),
            FixMode::Check,
        );
        assert_eq!(command.output_format, OutputFormat::Json);
    }
}
