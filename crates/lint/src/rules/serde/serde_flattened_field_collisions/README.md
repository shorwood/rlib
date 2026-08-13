# serde_flattened_field_collisions

## What it does

Finds concrete wire-name collisions between ordinary fields and locally visible struct schemas
merged with `#[serde(flatten)]`.

## Why is this bad?

Flattening places multiple Rust fields in one wire namespace. Duplicate keys can overwrite data,
deserialize inconsistently across formats, or make a round trip select a different field.

## Example

```rust,ignore
#[derive(serde::Serialize, serde::Deserialize)]
struct Details { id: u64 }

#[derive(serde::Serialize, serde::Deserialize)]
struct Envelope { id: u64, #[serde(flatten)] details: Details }
```

## Use instead

Rename one of the colliding fields or preserve a nested namespace.

```rust,ignore
#[derive(serde::Serialize, serde::Deserialize)]
struct Envelope { envelope_id: u64, details: Details }
```
