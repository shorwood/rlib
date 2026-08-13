# thiserror_ambiguous_error_sources

## What it does

Finds derived thiserror types with multiple causal-looking fields whose resolved local types also
derive `thiserror::Error`, while only one or none enters the standard source chain.

## Why is this bad?

The error stores several plausible causes but exposes at most one without explaining the policy.
Reporters lose part of the failure story and maintainers can accidentally change which cause is
primary.

## Example

```rust,ignore
#[derive(Debug, thiserror::Error)]
#[error("operation failed")]
struct OperationError {
    #[source]
    primary_error: ReadError,
    secondary_error: CleanupError,
}
```

## Use instead

Name one primary source explicitly and classify other failures as related, suppressed, fallback, or
aggregate errors according to domain policy.

```rust,ignore
#[derive(Debug, thiserror::Error)]
#[error("operation failed")]
struct OperationError {
    #[source]
    source: ReadError,
    suppressed_cleanup: CleanupError,
}
```
