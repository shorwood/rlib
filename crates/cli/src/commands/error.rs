//! Human-readable reports and stable command statuses.

use color_eyre::Section as _;
use color_eyre::eyre::{Report, Result};

/// Fallible result returned by an executable command.
pub type CommandResult<T = ()> = Result<T>;

// -----------------------------------------------------------------------------
// ToolFailure: Tooling and project policy
// -----------------------------------------------------------------------------

/// Marker for a required-tool, configuration, or protocol failure.
#[derive(Debug, derive_more::Display, derive_more::Error)]
#[display("{message}")]
struct ToolFailure {
    /// Human-readable tool failure context.
    message: String,
}

// -----------------------------------------------------------------------------
// FindingsFailure: Enforcing diagnostics
// -----------------------------------------------------------------------------

/// Marker for external findings configured to fail the invocation.
#[derive(Debug, derive_more::Display, derive_more::Error)]
#[display("SQLFluff found enforcing violations")]
struct FindingsFailure;

// -----------------------------------------------------------------------------
// CompilerFailure: Native analysis
// -----------------------------------------------------------------------------

/// Marker for a compiler-backed native analysis failure.
#[derive(Debug, derive_more::Display, derive_more::Error)]
#[display("{message}")]
struct CompilerFailure {
    /// Human-readable compiler failure context.
    message: String,
}

// -----------------------------------------------------------------------------
// Failure: Report construction
// -----------------------------------------------------------------------------

/// Constructs reports with the command status implied by their atomic cause.
pub(super) struct Failure;

impl Failure {
    /// Builds a required-tool, configuration, or protocol failure.
    pub(super) fn tool(message: impl Into<String>) -> Report {
        Report::new(ToolFailure {
            message: message.into(),
        })
        .suppress_backtrace(true)
    }

    /// Builds a compiler-analysis failure.
    pub(super) fn compiler(message: impl Into<String>) -> Report {
        Report::new(CompilerFailure {
            message: message.into(),
        })
        .suppress_backtrace(true)
    }

    /// Builds an enforcing-`SQLFluff`-findings failure.
    pub(super) fn findings() -> Report {
        Report::new(FindingsFailure).suppress_backtrace(true)
    }
}

// -----------------------------------------------------------------------------
// CommandReportExt: Stable process status
// -----------------------------------------------------------------------------

/// Process behavior derived from atomic failure markers in a report.
pub trait CommandReportExt {
    /// Returns the stable process status for this failure category.
    fn exit_code(&self) -> u8;
}

impl CommandReportExt for Report {
    fn exit_code(&self) -> u8 {
        if self.downcast_ref::<FindingsFailure>().is_some() {
            1
        } else if self.downcast_ref::<CompilerFailure>().is_some() {
            101
        } else {
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CommandReportExt as _, Failure};

    #[test]
    fn derives_status_from_atomic_failure_markers() {
        assert_eq!(Failure::findings().exit_code(), 1);
        assert_eq!(Failure::tool("tool failed").exit_code(), 2);
        assert_eq!(Failure::compiler("compiler failed").exit_code(), 101);
    }
}
