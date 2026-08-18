# `rlib::thiserror_dynamic_errors_in_library_interfaces`

## Summary

Finds public functions whose `Result` error type is `Box<dyn Error>`, `anyhow::Error`, or `miette::Report`.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::thiserror` |
| Cargo feature | `thiserror` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds public functions whose `Result` error type is `Box<dyn Error>`, `anyhow::Error`, or
`miette::Report`.

## Why this matters

Callers of a library API need a stable error type for recovery, classification, and testing.
Dynamic application errors erase that information and make behavior depend on downcasting or display
text.

## Examples

### Triggers the lint

```rust,ignore
pub fn load() -> Result<Data, Box<dyn std::error::Error>> { todo!() }
```

### Use this instead

Return a concrete thiserror domain type and retain dynamic errors only as deliberately opaque
sources where needed.

```rust,ignore
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("input failed")]
    Input(#[source] std::io::Error),
}

pub fn load() -> Result<Data, LoadError> { todo!() }
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
