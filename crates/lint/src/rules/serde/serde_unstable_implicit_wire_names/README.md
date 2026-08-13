# serde_unstable_implicit_wire_names

## What it does

Finds public derived Serde contracts whose named fields or enum variants implicitly use Rust
identifiers as wire names.

## Why is this bad?

An ordinary Rust rename then silently changes externally observed data. Explicit Serde naming makes
that compatibility decision visible and keeps refactors from rewriting a protocol or persisted
schema.

## Example

```rust,ignore
#[derive(serde::Serialize, serde::Deserialize)]
pub enum ApiEvent { UserCreated, UserDeleted }
```

## Use instead

Declare the wire vocabulary independently of Rust identifiers.

```rust,ignore
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiEvent { UserCreated, UserDeleted }
```
