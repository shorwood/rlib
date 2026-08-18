# `rlib::serde_duplicate_serialized_names`

## Summary

Finds fields or variants whose effective Serde names collide after directional renames, aliases, container case conversion, and directional skips.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::serde` |
| Cargo feature | `serde` |
| Purpose | Correctness |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds fields or variants whose effective Serde names collide after directional renames, aliases,
container case conversion, and directional skips.

## Why this matters

Duplicate output keys produce an ambiguous wire representation. Duplicate input names make member
selection order-dependent or cause deserialization failures that are disconnected from the schema.

## Examples

### Triggers the lint

```rust,ignore
#[derive(serde::Serialize, serde::Deserialize)]
struct Record {
    #[serde(rename = "id")]
    internal_id: u64,
    id: String,
}
```

### Use this instead

Give every serialized and accepted deserialized member a distinct explicit name.

```rust,ignore
struct Record {
    #[serde(rename = "internal_id")]
    internal_id: u64,
    id: String,
}
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off only when the reported behavior is intentional and covered by tests.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

- [`rlib::serde_unstable_implicit_wire_names`](../serde_unstable_implicit_wire_names/README.md) — Makes public wire names explicit before they can drift.
- [`rlib::serde_flattened_field_collisions`](../serde_flattened_field_collisions/README.md) — Finds name collisions introduced by flattened structs.
