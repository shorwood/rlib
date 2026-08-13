# serde_catch_all_variants_hiding_schema_drift

## What it does

Finds `#[serde(other)]` variants in public enums with derived deserialization.

## Why is this bad?

At an externally visible boundary, a catch-all turns new or misspelled wire variants into one
generic state. That hides schema drift, loses the unknown spelling, and can accidentally select
permissive behavior.

## Example

```rust,ignore
#[derive(serde::Deserialize)]
pub enum Permission { Read, Write, #[serde(other)] Unknown }
```

## Use instead

Reject unknown variants at strict boundaries, or use a representation that deliberately preserves
the unknown value for observation or forwarding.

```rust,ignore
#[derive(serde::Deserialize)]
pub enum Permission { Read, Write }
```
