# thiserror_from_sources_without_context

## What it does

Finds transparent `#[from]` variants that receive failures from two or more differently named
operations in the same function.

## Why is this bad?

Automatic conversion collapses failures from different operations into one indistinguishable
variant. The source chain survives, but callers cannot tell which domain operation failed.

## Example

```rust,ignore
fn load() -> Result<Data, LoadError> {
    let header = read_header()?;
    let body = read_body()?;
    Ok(Data { header, body })
}
```

## Use instead

Use contextual variants or explicit `map_err` at the operation boundary.

```rust,ignore
fn load() -> Result<Data, LoadError> {
    let header = read_header().map_err(LoadError::Header)?;
    let body = read_body().map_err(LoadError::Body)?;
    Ok(Data { header, body })
}
```
