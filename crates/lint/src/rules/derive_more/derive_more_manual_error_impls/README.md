# derive_more_manual_error_impls

## What it does

Finds authored `std::error::Error` implementations whose complete behavior is reproducible by
`derive_more::Error`.

## Why is this bad?

Boilerplate error implementations obscure the exceptional cases where a type intentionally changes
its source chain or provides additional diagnostic data.

## Example

```rust,ignore
impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}
```

## Use instead

```rust,ignore
#[derive(Debug, derive_more::Display, derive_more::Error)]
struct LoadError {
    source: std::io::Error,
}
```
