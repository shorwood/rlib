# `rlib::miette_malformed_diagnostic_codes`

## Summary

Finds static Miette diagnostic codes that are neither namespaced lowercase Rust paths nor conventional uppercase letter-number identifiers.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::miette` |
| Cargo feature | `miette` |
| Purpose | Correctness |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds static Miette diagnostic codes that are neither namespaced lowercase Rust paths nor
conventional uppercase letter-number identifiers.

## Why this matters

Mixed casing and presentation-like code names tend to drift with user-facing language and make a
crate's machine identifiers incoherent.

## Examples

### Triggers the lint

```rust,ignore
#[diagnostic(code(Config::InvalidValue))]
```

### Use this instead

```rust,ignore
#[diagnostic(code(config::invalid_value))]
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
