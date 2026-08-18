# `rlib::thiserror_unreported_error_sources`

## Summary

Finds a single likely cause that is itself a local thiserror type but is omitted from the containing error's `Error::source` chain.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::thiserror` |
| Cargo feature | `thiserror` |
| Purpose | Correctness |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds a single likely cause that is itself a local thiserror type but is omitted from the containing
error's `Error::source` chain.

## Why this matters

The nested cause is displayed or stored but unavailable through `Error::source`, so reports and
debugging tools stop at the wrapper instead of reaching the original failure.

## Examples

### Triggers the lint

```rust,ignore
#[derive(Debug, thiserror::Error)]
#[error("request failed: {cause}")]
struct RequestError { cause: TransportError }
```

### Use this instead

Mark the causal field explicitly, adding `#[from]` only when automatic conversion is also the
intended policy.

```rust,ignore
#[derive(Debug, thiserror::Error)]
#[error("request failed")]
struct RequestError { #[source] cause: TransportError }
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

- [`rlib::thiserror_ambiguous_error_sources`](../thiserror_ambiguous_error_sources/README.md) — Finds errors with several competing source fields.
- [`rlib::thiserror_unpropagated_error_backtraces`](../thiserror_unpropagated_error_backtraces/README.md) — Checks that source backtraces remain available.
