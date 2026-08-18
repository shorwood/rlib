# `rlib::derive_more_ambiguous_derived_error_sources`

## Summary

Finds `derive_more::Error` structs and enum variants that implicitly select a conventionally named `source` field while another error-named field also looks like a cause.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::derive_more` |
| Cargo feature | `derive_more` |
| Purpose | Correctness |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds `derive_more::Error` structs and enum variants that implicitly select a conventionally named
`source` field while another error-named field also looks like a cause. Source policy is evaluated
per field scope; unrelated helper attributes do not resolve the ambiguity.

## Why this matters

The standard error chain exposes only one source. Relying on field-name inference hides which of
several plausible causes owns that public diagnostic behavior.

## Examples

### Triggers the lint

```rust,ignore
#[derive(Debug, derive_more::Display, derive_more::Error)]
struct RequestError {
    source: std::io::Error,
    retry_error: BackoffError,
}
```

### Use this instead

Mark the intended source explicitly and mark other cause-like fields as non-sources.

```rust,ignore
struct RequestError {
    #[error(source)]
    source: std::io::Error,
    #[error(not(source))]
    retry_error: BackoffError,
}
```

## What it skips

Source policy is evaluated per field scope; unrelated helper attributes do not resolve the ambiguity.

## When to turn it off

Turn this lint off only when the reported behavior is intentional and covered by tests.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
