# `rlib::thiserror_manual_from_error_variants`

## Summary

Finds manual `From<SourceError>` implementations that only construct a one-field source-bearing variant of a type already deriving `thiserror::Error`.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::thiserror` |
| Cargo feature | `thiserror` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds manual `From<SourceError>` implementations that only construct a one-field source-bearing
variant of a type already deriving `thiserror::Error`.

## Why this matters

The handwritten conversion duplicates the error-source policy and can drift from the variant. A
single `#[from]` annotation keeps construction and source chaining coherent.

## Examples

### Triggers the lint

```rust,ignore
impl From<std::io::Error> for LoadError {
    fn from(source: std::io::Error) -> Self { Self::Io(source) }
}
```

### Use this instead

Select the thiserror provider in configuration and annotate the source field.

```rust,ignore
#[derive(Debug, thiserror::Error)]
enum LoadError {
    #[error("I/O failed")]
    Io(#[from] std::io::Error),
}
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `error-variant-conversion-provider` | string | not set | Chooses which derive should generate conversions into error variants. Values: `thiserror_from`, `derive_more_from`. |

## Known limitations

No known implementation limitations.

## Related lints

None.
