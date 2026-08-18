# `rlib::thiserror_opaque_errors_exposing_representations`

## Summary

Finds public derived thiserror enums whose source-bearing variants directly expose concrete error types from another crate.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::thiserror` |
| Cargo feature | `thiserror` |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds public derived thiserror enums whose source-bearing variants directly expose concrete error
types from another crate.

## Why this matters

Callers can then depend on the foreign type when pattern matching. Dependency
upgrades, implementation changes, and source substitutions then require downstream API changes.

## Examples

### Triggers the lint

```rust,ignore
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("I/O failed")]
    Io(#[from] std::io::Error),
}
```

### Use this instead

Preserve an evolvable public boundary with an opaque wrapper or stable owned variants while retaining
the foreign failure as a private source.

```rust,ignore
#[derive(Debug, thiserror::Error)]
#[error("I/O failed")]
pub struct ApiError(#[source] std::io::Error);
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
