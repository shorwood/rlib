# thiserror_manual_from_error_variants

## What it does

Finds manual `From<SourceError>` implementations that only construct a one-field source-bearing
variant of a type already deriving `thiserror::Error`.

## Why is this bad?

The handwritten conversion duplicates the error-source policy and can drift from the variant. A
single `#[from]` annotation keeps construction and source chaining coherent.

## Example

```rust,ignore
impl From<std::io::Error> for LoadError {
    fn from(source: std::io::Error) -> Self { Self::Io(source) }
}
```

## Use instead

Select the thiserror provider in configuration and annotate the source field.

```rust,ignore
#[derive(Debug, thiserror::Error)]
enum LoadError {
    #[error("I/O failed")]
    Io(#[from] std::io::Error),
}
```
