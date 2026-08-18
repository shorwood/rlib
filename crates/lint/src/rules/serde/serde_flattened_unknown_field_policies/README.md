# `rlib::serde_flattened_unknown_field_policies`

## Summary

Finds derived Serde deserializers that combine a flattened field with `#[serde(deny_unknown_fields)]` on the same container.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::serde` |
| Cargo feature | `serde` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds derived Serde deserializers that combine a flattened field with
`#[serde(deny_unknown_fields)]` on the same container.

## Why this matters

Flattening merges another schema or extension map into the container's key namespace, while
`deny_unknown_fields` requires that namespace to be closed. Serde does not support enforcing both
policies together, so the declared strictness is misleading.

## Examples

### Triggers the lint

```rust,ignore
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    id: u64,
    #[serde(flatten)]
    extensions: std::collections::HashMap<String, String>,
}
```

### Use this instead

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

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
