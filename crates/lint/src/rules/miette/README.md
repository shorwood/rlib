# Miette-aware lints

Miette diagnostics are part error model and part user interface. These lints keep codes stable,
labels useful, source excerpts safe, and application-oriented reports out of reusable library APIs.
They recognize `miette::Diagnostic` derives and consider their attributes together, so a warning is
based on the complete diagnostic rather than one attribute in isolation.

## Closed proposals

### `miette_transparent_diagnostics_hiding_context`

No active lint is registered for this proposal. Miette 7.6 accepts
`#[diagnostic(transparent)]` only on a struct or variant with exactly one field and rejects combining
it with code, help, severity, URL, or other diagnostic arguments. The macro therefore rejects the
problem before this lint library could see it.

A wrapper type alone is not evidence that useful context was lost. Warning on every valid
transparent wrapper would punish an intentional and supported Miette pattern.
