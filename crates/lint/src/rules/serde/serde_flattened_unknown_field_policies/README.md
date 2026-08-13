# serde_flattened_unknown_field_policies

## What it does

Finds derived Serde deserializers that combine a flattened field with
`#[serde(deny_unknown_fields)]` on the same container.

## Why is this bad?

Flattening merges another schema or extension map into the container's key namespace, while
`deny_unknown_fields` requires that namespace to be closed. Serde does not support enforcing both
policies together, so the declared strictness is misleading.

## Example

```rust,ignore
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    id: u64,
    #[serde(flatten)]
    extensions: std::collections::HashMap<String, String>,
}
```

## Use instead

Choose one policy at the boundary: reject unknown keys in a closed structure, or capture extensions
without `deny_unknown_fields`.

```rust,ignore
#[derive(serde::Deserialize)]
struct Request {
    id: u64,
    #[serde(flatten)]
    extensions: std::collections::HashMap<String, String>,
}
```
