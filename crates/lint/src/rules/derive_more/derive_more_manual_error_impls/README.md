# `rlib::derive_more_manual_error_impls`

## Summary

Finds hand-written `std::error::Error` implementations whose complete behavior is reproducible by `derive_more::Error`.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::derive_more` |
| Cargo feature | `derive_more` |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds hand-written `std::error::Error` implementations whose complete behavior is reproducible by
`derive_more::Error`.

## Why this matters

Boilerplate error implementations obscure the exceptional cases where a type intentionally changes
its source chain or provides additional diagnostic data.

## Examples

### Triggers the lint

```rust,ignore
impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}
```

### Use this instead

```rust,ignore
#[derive(Debug, derive_more::Display, derive_more::Error)]
struct LoadError {
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
| `error-implementation-provider` | string | not set | Chooses which derive should replace a hand-written `Error` implementation. Values: `derive_more_error`, `thiserror_error`. |

## Known limitations

No known implementation limitations.

## Related lints

None.
