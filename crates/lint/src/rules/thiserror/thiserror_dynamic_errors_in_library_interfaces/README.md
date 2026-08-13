# thiserror_dynamic_errors_in_library_interfaces

## What it does

Finds public functions whose `Result` error type is `Box<dyn Error>`, `anyhow::Error`, or
`miette::Report`.

## Why is this bad?

Callers of a library API need a stable error contract for recovery, classification, and testing.
Dynamic application errors erase that contract and make behavior depend on downcasting or display
text.

## Example

```rust,ignore
pub fn load() -> Result<Data, Box<dyn std::error::Error>> { todo!() }
```

## Use instead

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
