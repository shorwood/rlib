# serde_ambiguous_untagged_enums

## What it does

Finds pairs of named-field variants in a derived `#[serde(untagged)]` enum for which a concrete input
object can deserialize as either variant.

## Why is this bad?

Serde selects the first matching untagged variant. Overlapping shapes therefore make declaration
order part of the data contract and can silently reinterpret input after a reorder or schema change.

## Example

```rust,ignore
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum Value {
    Detailed { id: u64, #[serde(default)] name: String },
    Minimal { id: u64 },
}
```

## Use instead

Use an explicit tag or make the accepted shapes structurally disjoint.

```rust,ignore
#[derive(serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Value {
    Detailed { id: u64, name: String },
    Minimal { id: u64 },
}
```
