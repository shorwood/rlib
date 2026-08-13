# thiserror_manual_error_impls

## What it does

Finds authored `Display` and `std::error::Error` implementations whose complete behavior is
reproducible by `thiserror::Error`.

## Why is this bad?

Separate implementations scatter an error's message and source-chain policy. Thiserror attributes
keep that contract beside the error declaration while leaving custom implementations alone.

## Example

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

## Use instead

```rust,ignore
#[derive(Debug, thiserror::Error)]
#[error("load failed")]
struct LoadError {
    #[source]
    source: std::io::Error,
}
```
