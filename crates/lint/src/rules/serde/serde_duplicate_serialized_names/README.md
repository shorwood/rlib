# serde_duplicate_serialized_names

## What it does

Finds fields or variants whose effective Serde names collide after directional renames, aliases,
container case conversion, and directional skips.

## Why is this bad?

Duplicate output keys produce an ambiguous wire representation. Duplicate input names make member
selection order-dependent or cause deserialization failures that are disconnected from the schema.

## Example

```rust,ignore
#[derive(serde::Serialize, serde::Deserialize)]
struct Record {
    #[serde(rename = "id")]
    internal_id: u64,
    id: String,
}
```

## Use instead

Give every serialized and accepted deserialized member a distinct explicit name.

```rust,ignore
struct Record {
    #[serde(rename = "internal_id")]
    internal_id: u64,
    id: String,
}
```
