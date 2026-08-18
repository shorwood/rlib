# `rlib::thiserror_unpropagated_error_backtraces`

## Summary

Finds local thiserror wrappers that recapture a backtrace already provided by their source or fail to forward that source backtrace.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::thiserror` |
| Cargo feature | `thiserror` |
| Purpose | Safety |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds local thiserror wrappers that recapture a backtrace already provided by their source or fail to
forward that source backtrace.

## Why this matters

Recapturing adds cost and points at the wrapper instead of the original failure. A plain
`#[source]` field keeps the error chain, but tools still cannot retrieve the source's backtrace
through the wrapper.

## Examples

### Triggers the lint

```rust,ignore
#[derive(Debug, thiserror::Error)]
#[error("outer")]
struct Outer {
    #[source]
    source: Inner,
    backtrace: std::backtrace::Backtrace,
}
```

### Use this instead

```rust,ignore
#[derive(Debug, thiserror::Error)]
#[error("outer")]
struct Outer {
    #[source]
    #[backtrace]
    source: Inner,
}
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off only when the risk is handled elsewhere and documented.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

- [`rlib::thiserror_unreported_error_sources`](../thiserror_unreported_error_sources/README.md) — Checks that the source error itself remains available.
