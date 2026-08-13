# thiserror-aware lints

Good library errors should be easy for people to read and easy for callers to classify. These lints
check `thiserror::Error` derives for clear messages, correct source chains, safe public boundaries,
and conversion shortcuts that discard context. They also identify hand-written error code that the
derive can replace without changing behavior.

## Closed proposals

### `thiserror_transparent_errors_hiding_context`

No active lint is registered for this proposal. In thiserror 2.x, `#[error(transparent)]` is accepted
only for a struct or variant containing exactly one field. The derive macro already rejects a
transparent source combined with extra context, so a lint would never see that program.

A wrapper type alone does not prove that useful context was hidden. Warning on every transparent
wrapper would penalize a valid way to keep implementation details private.

### `thiserror_diagnostic_sources_downgraded_to_errors`

No thiserror lint is registered for this proposal. The same condition is owned by
`miette_plain_error_diagnostic_sources`, which can require both the containing and source types to
implement `miette::Diagnostic` before recommending `#[diagnostic_source]`.

Keeping the rule in Miette avoids two warnings for the same field and leaves thiserror focused on
the standard error chain. There is only one useful fix here, so no provider-selection setting is
needed.
