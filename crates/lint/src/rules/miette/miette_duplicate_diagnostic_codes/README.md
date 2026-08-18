# `rlib::miette_duplicate_diagnostic_codes`

## Summary

Finds distinct Miette diagnostic types or enum variants that expose the same static diagnostic code.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::miette` |
| Cargo feature | `miette` |
| Purpose | Correctness |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds distinct Miette diagnostic types or enum variants that expose the same static diagnostic code.

## Why this matters

Diagnostic codes are machine identity used by support, filtering, and telemetry. Reusing one code for
different failures prevents those tools and callers from telling the failures apart.

## Examples

### Triggers the lint

```rust,ignore
#[derive(Debug, thiserror::Error, miette::Diagnostic)]
enum ConfigError {
    #[error("missing value")]
    #[diagnostic(code(config::invalid))]
    Missing,
    #[error("invalid value")]
    #[diagnostic(code(config::invalid))]
    Invalid,
}
```

### Use this instead

```rust,ignore
#[derive(Debug, thiserror::Error, miette::Diagnostic)]
enum ConfigError {
    #[error("missing value")]
    #[diagnostic(code(config::missing))]
    Missing,
    #[error("invalid value")]
    #[diagnostic(code(config::invalid))]
    Invalid,
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

None.
