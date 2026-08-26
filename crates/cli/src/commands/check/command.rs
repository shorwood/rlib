//! Default `cargo rlib` analysis command.

use color_eyre::Section as _;

use super::compiler::{self, Invocation, Workspace};
use super::diagnostics::Emitter;
use super::mode::FixMode;
use crate::commands::error::{CommandResult, Failure};
use crate::commands::sqlfluff::{BundledSqlfluff, PolicyScope, QueryOriginExt as _};

// -----------------------------------------------------------------------------
// Request: Complete analysis operation
// -----------------------------------------------------------------------------

/// One complete native and configured-adapter analysis operation.
pub struct Request {
    /// Arguments interpreted by Cargo for package and target selection.
    cargo_arguments: Vec<String>,
    /// Arguments forwarded directly to rustc after the public delimiter.
    rustc_arguments: Vec<String>,
    /// Compiler mutation policy for the optional first pass.
    fix_mode: FixMode,
}

impl Request {
    /// Creates an analysis request from arguments separated by the CLI parser.
    pub const fn new(
        cargo_arguments: Vec<String>,
        rustc_arguments: Vec<String>,
        fix_mode: FixMode,
    ) -> Self {
        Self {
            cargo_arguments,
            rustc_arguments,
            fix_mode,
        }
    }

    /// Runs native checks, configured SQL analysis, and optional native fixes.
    pub fn run(self) -> CommandResult {
        let workspace = Workspace::discover()?;
        let invocation = Invocation::new(self.cargo_arguments, self.rustc_arguments);
        if matches!(self.fix_mode, FixMode::Fix) {
            workspace.fix(&invocation)?;
        }

        // The final non-mutating pass both checks native lints and recovers authored queries.
        let queries = workspace.extract(&invocation)?;
        let configured = queries
            .iter()
            .filter(|query| {
                let origin = query.document.origin.authored_path();
                let policy_root = origin.parent().unwrap_or_else(|| workspace.root());
                PolicyScope::new(policy_root, workspace.root()).is_configured()
            })
            .collect::<Vec<_>>();

        // Native analysis is complete when no authored SQL policy opts in.
        if configured.is_empty() {
            return Ok(());
        }

        // Applicable project policy opts a query into the bundled SQL backend.
        let backend = BundledSqlfluff::load()?;
        for query in &configured {
            let origin = query.document.origin.authored_path();
            let policy_root = origin.parent().unwrap_or_else(|| workspace.root());

            // An opted-in document cannot be parsed without a declared dialect.
            if !PolicyScope::new(policy_root, workspace.root()).has_dialect() {
                return Err(
                    Failure::tool(format!(
                        "SQLFluff configuration applying to `{}` does not declare a dialect",
                        origin.display()
                    ))
                    .suggestion(
                        "Declare `dialect = postgres`, `mysql`, or `sqlite` in the applicable SQLFluff configuration.",
                    ),
                );
            }
        }

        let emitter = Emitter::new(invocation.cargo_arguments(), workspace.metadata());
        let mut has_enforcing_findings = false;
        for query in configured {
            for finding in backend.lint(&query.document)? {
                let severity = emitter.emit(query, &finding)?;
                has_enforcing_findings |= severity.is_enforcing();
            }
        }
        if has_enforcing_findings {
            Err(Failure::findings())
        } else {
            Ok(())
        }
    }
}

// -----------------------------------------------------------------------------
// CompilerChildRequest: Private process dispatch
// -----------------------------------------------------------------------------

/// One private compiler-runner child invocation.
pub struct CompilerChildRequest {
    /// Compiler mutation policy selected by the parent process.
    fix_mode: FixMode,
}

impl CompilerChildRequest {
    /// Captures the private runner option parsed by Clap.
    pub const fn new(fix_mode: FixMode) -> Self {
        Self { fix_mode }
    }

    /// Runs Dylint inside the isolated child process.
    pub fn run(self) -> CommandResult {
        compiler::run_internal(self.fix_mode)
    }
}

#[cfg(test)]
mod tests {
    use super::Request;
    use crate::commands::check::mode::FixMode;

    #[test]
    fn retains_separated_command_arguments() {
        let request = Request::new(
            vec!["--workspace".to_owned()],
            vec!["-D".to_owned(), "warnings".to_owned()],
            FixMode::Fix,
        );
        assert!(matches!(request.fix_mode, FixMode::Fix));
        assert_eq!(request.cargo_arguments, ["--workspace"]);
        assert_eq!(request.rustc_arguments, ["-D", "warnings"]);
    }
}
