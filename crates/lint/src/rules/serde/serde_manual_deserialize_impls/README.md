# serde_manual_deserialize_impls

## What it does

Finds `Deserialize` implementations on single-field newtypes that decode the field unchanged and
immediately construct `Self`.

## Why is this bad?

Transparent decoding boilerplate obscures the manual visitors that intentionally validate,
normalize, accept legacy forms, or provide richer errors.

## Example

```rust,ignore
impl<'de> serde::Deserialize<'de> for UserId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: serde::Deserializer<'de> {
        Ok(Self(u64::deserialize(deserializer)?))
    }
}
```

## Use instead

```rust,ignore
#[derive(serde::Deserialize)]
#[serde(transparent)]
struct UserId(u64);
```
