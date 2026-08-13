# thiserror_ambiguous_error_sources

## What it does

Finds derived thiserror types that store several likely causes, even though at most one is exposed
through `Error::source`.

## Why is this bad?

An error may need a primary cause plus related or suppressed failures, but those roles should be
clear. Otherwise reports lose part of the failure story and a field rename can accidentally change
which cause callers see.

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
