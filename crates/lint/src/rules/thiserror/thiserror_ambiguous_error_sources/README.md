# `rlib::thiserror_ambiguous_error_sources`

## Summary

Finds derived thiserror types that store several likely causes, even though at most one is exposed through `Error::source`.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::thiserror` |
| Cargo feature | `thiserror` |
| Purpose | Correctness |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds derived thiserror types that store several likely causes, even though at most one is exposed
through `Error::source`.

## Why this matters

An error may need a primary cause plus related or suppressed failures, but those roles should be
clear. Otherwise reports lose part of the failure story and a field rename can accidentally change
which cause callers see.

## Examples

### Triggers the lint

```rust,ignore
#[derive(Debug, thiserror::Error)]
#[error("operation failed")]
struct OperationError {
    #[source]
    primary_error: ReadError,
    secondary_error: CleanupError,
}
```

### Use this instead

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

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off only when the reported behavior is intentional and covered by tests.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

- [`rlib::thiserror_unreported_error_sources`](../thiserror_unreported_error_sources/README.md) — Finds a likely source that is not reported at all.
- [`rlib::thiserror_from_sources_without_context`](../thiserror_from_sources_without_context/README.md) — Finds source conversions that lose operation context.
