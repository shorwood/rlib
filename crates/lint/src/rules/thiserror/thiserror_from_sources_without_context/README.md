# `rlib::thiserror_from_sources_without_context`

## Summary

Finds transparent `#[from]` variants that receive failures from two or more differently named operations in the same function.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::thiserror` |
| Cargo feature | `thiserror` |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds transparent `#[from]` variants that receive failures from two or more differently named
operations in the same function.

## Why this matters

Automatic conversion collapses failures from different operations into one indistinguishable
variant. The source chain survives, but callers cannot tell which domain operation failed.

## Examples

### Triggers the lint

```rust,ignore
fn load() -> Result<Data, LoadError> {
    let header = read_header()?;
    let body = read_body()?;
    Ok(Data { header, body })
}
```

### Use this instead

Use contextual variants or explicit `map_err` at the operation boundary.

```rust,ignore
fn load() -> Result<Data, LoadError> {
    let header = read_header().map_err(LoadError::Header)?;
    let body = read_body().map_err(LoadError::Body)?;
    Ok(Data { header, body })
}
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
