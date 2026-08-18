# `rlib::miette_ad_hoc_diagnostics_at_domain_boundaries`

## Summary

Finds the same static ad-hoc Miette diagnostic or context message propagated by multiple functions when at least one occurrence crosses a public API boundary.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::miette` |
| Cargo feature | `miette` |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds the same static ad-hoc Miette diagnostic or context message propagated by multiple functions
when at least one occurrence crosses a public API boundary.
Both eager and lazy resolved `WrapErr` contexts are included; unrelated methods with the same name
are excluded.

## Why this matters

A repeated public failure is part of the domain names. Keeping it as anonymous text prevents
callers and machine reporting from identifying it reliably, and makes its metadata drift between
construction sites.

## Examples

### Triggers the lint

```rust,ignore
pub fn load_primary() -> miette::Result<()> {
    Err(miette::miette!("configuration is missing"))
}

pub fn load_fallback() -> miette::Result<()> {
    Err(miette::miette!("configuration is missing"))
}
```

### Use this instead

```rust,ignore
#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("configuration is missing")]
#[diagnostic(code(config::missing))]
struct MissingConfiguration;
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when the reported API is deliberate and callers depend on it.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
