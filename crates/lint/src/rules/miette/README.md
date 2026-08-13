# Miette lint design decisions

This layer recognizes `miette::Diagnostic` derives by macro provenance and models diagnostic
metadata, field roles, source paths, and public reporting boundaries as one contract.

## Closed proposals

### `miette_transparent_diagnostics_hiding_context`

No active lint is registered for this proposal. Miette 7.6 accepts
`#[diagnostic(transparent)]` only on a struct or variant with exactly one field and rejects combining
`transparent` with code, help, severity, URL, or other diagnostic arguments. Consequently, a valid
transparent derive cannot also own the stored context or metadata the proposal intended to detect.

Wrapper type identity alone is insufficient evidence of hidden actionable context, and pure
representation-hiding wrappers are explicitly accepted. Warning on every valid transparent wrapper
would therefore reverse the proposal's intended policy.
