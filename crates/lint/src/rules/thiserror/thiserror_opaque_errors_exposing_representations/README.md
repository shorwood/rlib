# thiserror_opaque_errors_exposing_representations

## What it does

Finds public derived thiserror enums whose source-bearing variants directly expose concrete error
types from another crate.

## Why is this bad?

The foreign representation becomes part of the public pattern-matching contract. Dependency
upgrades, implementation changes, and source substitutions then require downstream API changes.

## Example

```rust,ignore
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("I/O failed")]
    Io(#[from] std::io::Error),
}
```

## Use instead

Preserve an evolvable public boundary with an opaque wrapper or stable owned variants while retaining
the foreign failure as a private source.

```rust,ignore
#[derive(Debug, thiserror::Error)]
#[error("I/O failed")]
pub struct ApiError(#[source] std::io::Error);
```
