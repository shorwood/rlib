# `rlib::thiserror_manual_error_impls`

## Summary

Finds hand-written `Display` and `std::error::Error` implementations whose complete behavior is reproducible by `thiserror::Error`.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::thiserror` |
| Cargo feature | `thiserror` |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds hand-written `Display` and `std::error::Error` implementations whose complete behavior is
reproducible by `thiserror::Error`.

## Why this matters

Separate implementations spread an error's message and source behavior across several places.
Thiserror attributes keep both beside the error declaration while leaving custom code alone.

## Examples

### Triggers the lint

```rust,ignore
impl std::fmt::Display for LoadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("load failed")
    }
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}
```

### Use this instead

```rust,ignore
#[derive(Debug, thiserror::Error)]
#[error("load failed")]
struct LoadError {
    #[source]
    source: std::io::Error,
}
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when the reported API is deliberate and callers depend on it.

## Settings

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `error-implementation-provider` | string | not set | Chooses which derive should replace a hand-written `Error` implementation. Values: `thiserror_error`, `derive_more_error`. |

## Known limitations

No known implementation limitations.

## Related lints

None.
