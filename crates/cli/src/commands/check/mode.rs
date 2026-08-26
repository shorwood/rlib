//! Compiler mutation policy shared by command and compiler execution.

/// Parsed presence of the public `--fix` option.
#[derive(Clone, Copy, derive_more::From)]
pub struct FixRequest {
    /// Whether the invocation requested compiler mutation.
    is_requested: bool,
}

/// Whether a compiler pass checks source or applies native suggestions.
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
