# thiserror lint design decisions

This layer recognizes `thiserror::Error` derives by macro provenance and treats standard source,
conversion, presentation, and public-boundary behavior as one error contract.

## Closed proposals

### `thiserror_transparent_errors_hiding_context`

No active lint is registered for this proposal. In thiserror 2.x, `#[error(transparent)]` is accepted
only for a struct or variant containing exactly one field. A transparent source cannot therefore
coexist with an additional context or policy-marker field in a compiling derived error—the derive
macro already rejects the state the proposed lint would diagnose.

Type identity by itself does not provide the proposal's stronger evidence of hidden stored context,
and deliberately opaque public wrappers were explicitly accepted. Warning on every valid transparent
wrapper would consequently invert the proposal's intended boundary policy rather than enforce it.

### `thiserror_diagnostic_sources_downgraded_to_errors`

No thiserror lint is registered for this proposal. The same condition is owned by
`miette_plain_error_diagnostic_sources`, which can require both the containing and source types to
implement `miette::Diagnostic` before recommending `#[diagnostic_source]`.

Keeping the rule in Miette avoids duplicate diagnostics for one field and keeps thiserror's layer
focused on standard error-chain semantics. This is ownership consolidation, not provider selection:
there are not two alternative remediations, so no derive-resolution knob is involved.
