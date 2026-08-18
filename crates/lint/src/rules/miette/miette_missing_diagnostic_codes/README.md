# `rlib::miette_missing_diagnostic_codes`

## Summary

Finds non-transparent variants without codes in Miette diagnostic enums whose siblings establish a code policy.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::miette` |
| Cargo feature | `miette` |
| Purpose | Correctness |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds non-transparent variants without codes in Miette diagnostic enums whose siblings establish a
code policy.

## Why this matters

Partially coded families make otherwise comparable failures impossible to filter, track, or identify
in machine-oriented output.

## Examples

### Triggers the lint

```rust,ignore
#[derive(Debug, thiserror::Error, miette::Diagnostic)]
enum ConfigError {
    #[error("missing")]
    #[diagnostic(code(config::missing))]
    Missing,
    #[error("invalid")]
    Invalid,
}
```

### Use this instead

```rust,ignore
#[error("invalid")]
#[diagnostic(code(config::invalid))]
Invalid,
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

None.
