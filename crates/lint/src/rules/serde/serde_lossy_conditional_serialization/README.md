# serde_lossy_conditional_serialization

## What it does

Finds conditionally serialized required fields that have neither optionality nor a deserialization
default.

## Why is this bad?

The serializer can omit the field, but the same type's deserializer rejects the resulting missing
key. A value can therefore fail to round-trip solely because it satisfies the skip predicate.

## Example

```rust,ignore
#[derive(serde::Serialize, serde::Deserialize)]
struct Profile {
    #[serde(skip_serializing_if = "String::is_empty")]
    display_name: String,
}
```

## Use instead

Make omission reversible with optionality or a matching default policy.

```rust,ignore
struct Profile {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    display_name: String,
}
```
