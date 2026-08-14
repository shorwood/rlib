# miette_ad_hoc_diagnostics_at_domain_boundaries

## What it does

Finds the same static ad-hoc Miette diagnostic or context message propagated by multiple functions
when at least one occurrence crosses a public API boundary.
Both eager and lazy resolved `WrapErr` contexts are included; unrelated methods with the same name
are excluded.

## Why is this bad?

A repeated public failure is part of the domain vocabulary. Keeping it as anonymous text prevents
callers and machine reporting from identifying it reliably, and makes its metadata drift between
construction sites.

## Example

```rust,ignore
pub fn load_primary() -> miette::Result<()> {
    Err(miette::miette!("configuration is missing"))
}

pub fn load_fallback() -> miette::Result<()> {
    Err(miette::miette!("configuration is missing"))
}
```

## Use instead

```rust,ignore
#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("configuration is missing")]
#[diagnostic(code(config::missing))]
struct MissingConfiguration;
```
