//! Compiler-backed lint execution and query extraction.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs};

use cargo_metadata::{Metadata, MetadataCommand};
use color_eyre::eyre::WrapErr as _;
use color_eyre::{Section as _, SectionExt as _};
use dylint::opts::{Check, Dylint, LibrarySelection, Operation};
use tempfile::{Builder, TempDir};

use super::manifest::{ExtractedQuery, Loader};
use super::mode::FixMode;
use crate::commands::error::{CommandResult, Failure};

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

/// Dylint environment variable locating compiler drivers by toolchain.
const INTERNAL_ENV_DYLINT_DRIVERS: &str = "DYLINT_DRIVER_PATH";

/// Dylint environment variable carrying flags forwarded directly to rustc.
const INTERNAL_ENV_DYLINT_RUSTFLAGS: &str = "DYLINT_RUSTFLAGS";

/// Rust toolchain compatibility key used to select a Dylint driver.
const INTERNAL_ENV_TOOLCHAIN: &str = "RUSTUP_TOOLCHAIN";

// -----------------------------------------------------------------------------
// Workspace: Cargo analysis boundary
// -----------------------------------------------------------------------------

/// Cargo workspace being analyzed by one command invocation.
pub(super) struct Workspace {
    /// Cargo package and target identities used by machine diagnostics.
    metadata: Metadata,
    /// Canonical workspace root used as the source trust boundary.
    root: PathBuf,
}

impl Workspace {
    /// Discovers the Cargo workspace containing the current invocation.
    pub(super) fn discover() -> CommandResult<Self> {
        let metadata = MetadataCommand::new()
            .no_deps()
            .exec()
            .map_err(|error| Failure::tool(format!("could not read Cargo metadata: {error}")))?;
        let root = metadata
            .workspace_root
            .clone()
            .into_std_path_buf()
            .canonicalize()?;
        Ok(Self { metadata, root })
    }

    /// Returns Cargo metadata for diagnostic identity resolution.
    pub(super) const fn metadata(&self) -> &Metadata {
        &self.metadata
    }

    /// Returns the canonical source trust boundary.
    pub(super) fn root(&self) -> &Path {
        &self.root
    }

    /// Runs the private Dylint child with optional compiler-backed query extraction.
    fn run_compiler(
        &self,
        invocation: &Invocation,
        output: Option<&Path>,
        mode: FixMode,
    ) -> CommandResult {
        let temporary_root = self.root.join("target/rlib/sqlx");
        fs::create_dir_all(&temporary_root).wrap_err_with(|| {
            format!(
                "could not create compiler workspace `{}`",
                temporary_root.display()
            )
        })?;
        let driver = CompilerDriverAlias::create(&temporary_root)?;
        let mut command = Command::new(env::current_exe()?);
        command.current_dir(self.root());
        command.arg("__run-dylint");
        command.env(INTERNAL_ENV_DYLINT_DRIVERS, driver.path());
        if matches!(mode, FixMode::Fix) {
            command.arg("--fix");
        }
        command.env(
            INTERNAL_ENV_CARGO_ARGUMENTS,
            serde_json::to_string(&invocation.cargo_arguments).map_err(|error| {
                Failure::tool(format!("could not serialize Cargo arguments: {error}"))
            })?,
        );
        invocation.forward_rustc_arguments(&mut command)?;
        if let Some(output) = output {
            command.env(EXTRACTION_ENV_MANIFEST, output);
            command.env(EXTRACTION_ENV_WORKSPACE, self.root());
        }
        let status = command.status()?;
        if status.success() {
            Ok(())
        } else {
            Err(Failure::compiler(format!(
                "SQLx compiler analysis failed with {status}"
            )))
        }
    }

    /// Applies native machine-applicable suggestions before the final check pass.
    pub(super) fn fix(&self, invocation: &Invocation) -> CommandResult {
        self.run_compiler(invocation, None, FixMode::Fix)
    }

    /// Runs a non-mutating compiler pass and returns its trusted query documents.
    pub(super) fn extract(&self, invocation: &Invocation) -> CommandResult<Vec<ExtractedQuery>> {
        let temporary_root = self.root.join("target/rlib/sqlx");
        fs::create_dir_all(&temporary_root).wrap_err_with(|| {
            format!(
                "could not create compiler workspace `{}`",
                temporary_root.display()
            )
        })?;
        let output = Builder::new().prefix("run-").tempdir_in(temporary_root)?;
        self.run_compiler(invocation, Some(output.path()), FixMode::Check)?;
        Loader::new(output.path(), self.root()).load()
    }
}

// -----------------------------------------------------------------------------
// Invocation: Compiler argument ownership
// -----------------------------------------------------------------------------

/// Arguments forwarded across the Cargo and rustc protocol boundaries.
pub(super) struct Invocation {
    /// Arguments interpreted by Cargo for package and target selection.
    cargo_arguments: Vec<String>,
    /// Arguments forwarded directly to rustc.
    rustc_arguments: Vec<String>,
}

impl Invocation {
    /// Owns the arguments for all compiler passes in one check command.
    pub(super) const fn new(cargo_arguments: Vec<String>, rustc_arguments: Vec<String>) -> Self {
        Self {
            cargo_arguments,
            rustc_arguments,
        }
    }

    /// Returns the Cargo arguments used by this invocation.
    pub(super) fn cargo_arguments(&self) -> &[String] {
        &self.cargo_arguments
    }

    /// Combines inherited and requested rustc arguments for Dylint's textual protocol.
    fn forward_rustc_arguments(&self, command: &mut Command) -> CommandResult {
        // Without requested flags, the inherited Dylint policy remains untouched.
        if self.rustc_arguments.is_empty() {
            return Ok(());
        }
        let inherited = match env::var(INTERNAL_ENV_DYLINT_RUSTFLAGS) {
            Ok(flags) => flags,
            Err(env::VarError::NotPresent) => String::new(),
            // Non-Unicode flags cannot be combined without silently changing policy.
            Err(error @ env::VarError::NotUnicode(_)) => {
                return Err(Failure::tool(format!(
                    "inherited Dylint rustc flags are invalid: {error}"
                )));
            }
        };
        let requested = self.rustc_arguments.join(" ");
        let flags = if inherited.is_empty() {
            requested
        } else {
            format!("{inherited} {requested}")
        };
        command.env(INTERNAL_ENV_DYLINT_RUSTFLAGS, flags);
        Ok(())
    }
}

/// Invokes Dylint through its library API from the private runner child.
pub(super) fn run_internal(mode: FixMode) -> CommandResult {
    let arguments = env::var(INTERNAL_ENV_CARGO_ARGUMENTS)
        .map_err(|error| Failure::tool(format!("missing internal Cargo arguments: {error}")))?;
    let arguments: Vec<String> = serde_json::from_str(&arguments)
        .map_err(|error| Failure::tool(format!("invalid internal Cargo arguments: {error}")))?;
    let lint_library = env::var(INTERNAL_ENV_LINT_LIBRARY).map_err(|_error| {
        Failure::tool("the native lint library is unavailable")
            .suggestion("Reinstall cargo-rlib to restore its bundled compiler components.")
    })?;
    let options = Dylint {
        operation: Operation::Check(Check {
            lib_sel: LibrarySelection {
                lib_paths: vec![lint_library],
                no_build: true,
                no_metadata: true,
                ..LibrarySelection::default()
            },
            fix: matches!(mode, FixMode::Fix),
            args: arguments,
            ..Check::default()
        }),
        ..Dylint::default()
    };
    dylint::run(&options).map_err(|error| {
        Failure::compiler("compiler analysis failed").section(format!("{error:#}").header("Cause:"))
    })
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
    fn create(temporary_root: &Path) -> CommandResult<Self> {
        let drivers = env::var_os(INTERNAL_ENV_DYLINT_DRIVERS).ok_or_else(|| {
            Failure::tool("the bundled compiler driver is unavailable")
                .suggestion("Reinstall cargo-rlib to restore its bundled compiler components.")
        })?;
        let toolchain = env::var_os(INTERNAL_ENV_TOOLCHAIN).ok_or_else(|| {
            Failure::tool("the bundled compiler toolchain identity is unavailable")
                .suggestion("Run cargo-rlib through its installed Cargo integration.")
        })?;
        let executable = format!("dylint-driver{}", env::consts::EXE_SUFFIX);
        let source = PathBuf::from(drivers).join(&toolchain).join(&executable);

        // A missing source cannot be repaired safely inside the installed package.
        if !source.is_file() {
            return Err(Failure::tool(format!(
                "bundled compiler driver `{}` does not exist",
                source.display()
            ))
            .suggestion("Reinstall cargo-rlib to restore its bundled compiler components."));
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
