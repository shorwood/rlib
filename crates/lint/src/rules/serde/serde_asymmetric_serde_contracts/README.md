# serde_asymmetric_serde_contracts

## What it does

Finds undocumented directional Serde renames whose serialization and deserialization names differ
on a type deriving both traits.

## Why is this bad?

The type writes one schema and reads another. Without an explicit compatibility explanation,
round-trip expectations and future migrations are unclear.

## Example

```rust,ignore
#[derive(serde::Serialize, serde::Deserialize)]
struct Account {
    #[serde(rename(serialize = "user_id", deserialize = "id"))]
    id: u64,
}
```

## Use instead

Use one stable name, or document the intentional migration bridge at the renamed declaration.

```rust,ignore
struct Account {
    #[serde(rename = "user_id")]
    id: u64,
}
```
