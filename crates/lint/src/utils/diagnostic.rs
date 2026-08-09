extern crate rustc_lint;

use std::borrow::Cow;

use rustc_lint::{EarlyContext, LateContext};

// -----------------------------------------------------------------------------
// EarlyViolation: Early lint diagnostic boundary
// -----------------------------------------------------------------------------

/// Complete early-lint violation that knows how to render its own diagnostic.
pub trait EarlyViolation {
    /// Describes the violated policy at the primary diagnostic location.
    fn primary_message(&self) -> Cow<'_, str>;

    /// Explains why the violation makes the code harder to understand or maintain.
    fn rationale_message(&self) -> Cow<'_, str>;

    /// Describes the preferred remediation, whether rendered as help or a suggestion.
    fn remediation_message(&self) -> Cow<'_, str>;

    /// Consumes the classified violation and emits its user-facing diagnostic.
    fn emit(self, cx: &EarlyContext<'_>);
}

// -----------------------------------------------------------------------------
// LateViolation: Late lint diagnostic boundary
// -----------------------------------------------------------------------------

/// Complete late-lint violation that knows how to render its own diagnostic.
pub trait LateViolation {
    /// Describes the violated policy at the primary diagnostic location.
    fn primary_message(&self) -> Cow<'_, str>;

    /// Explains why the violation makes the code harder to understand or maintain.
    fn rationale_message(&self) -> Cow<'_, str>;

    /// Describes the preferred remediation, whether rendered as help or a suggestion.
    fn remediation_message(&self) -> Cow<'_, str>;

    /// Consumes the classified violation and emits its user-facing diagnostic.
    fn emit(self, cx: &LateContext<'_>);
}
