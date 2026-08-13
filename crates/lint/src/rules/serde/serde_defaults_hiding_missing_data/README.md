# serde_defaults_hiding_missing_data

## What it does

Finds undocumented bare `#[serde(default)]` fields whose type is not `Option`.

## Why is this bad?

A missing input silently becomes the type's generic `Default`, which can turn absence into a
meaningful quantity, identifier, flag, path, or timestamp without expressing schema policy.

## Example

```rust,ignore
#[derive(serde::Deserialize)]
struct Limits {
    #[serde(default)]
    maximum: u32,
}
```

## Use instead

Use optionality or a named default that states the compatibility policy.

```rust,ignore
fn default_maximum() -> u32 { 100 }

struct Limits {
    #[serde(default = "default_maximum")]
    maximum: u32,
}
```
