# derive_more_ambiguous_derived_error_sources

## What it does

Finds `derive_more::Error` structs and enum variants that implicitly select a conventionally named
`source` field while another error-named field also looks like a cause. Source policy is evaluated
per field scope; unrelated helper attributes do not resolve the ambiguity.

## Why is this bad?

The standard error chain exposes only one source. Relying on field-name inference hides which of
several plausible causes owns that public diagnostic contract.

## Example

```rust,ignore
#[derive(Debug, derive_more::Display, derive_more::Error)]
struct RequestError {
    source: std::io::Error,
    retry_error: BackoffError,
}
```

## Use instead

Mark the intended source explicitly and mark other cause-like fields as non-sources.

```rust,ignore
struct RequestError {
    #[error(source)]
    source: std::io::Error,
    #[error(not(source))]
    retry_error: BackoffError,
}
```
