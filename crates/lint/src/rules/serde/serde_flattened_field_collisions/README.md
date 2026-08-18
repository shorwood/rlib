# `rlib::serde_flattened_field_collisions`

## Summary

Finds concrete wire-name collisions between ordinary fields and locally visible struct schemas merged with `#[serde(flatten)]`.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::serde` |
| Cargo feature | `serde` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds concrete wire-name collisions between ordinary fields and locally visible struct schemas
merged with `#[serde(flatten)]`.

## Why this matters

Flattening places multiple Rust fields in one wire namespace. Duplicate keys can overwrite data,
deserialize inconsistently across formats, or make a round trip select a different field.

## Examples

### Triggers the lint

```rust,ignore
#[derive(serde::Serialize, serde::Deserialize)]
struct Details { id: u64 }

#[derive(serde::Serialize, serde::Deserialize)]
struct Envelope { id: u64, #[serde(flatten)] details: Details }
```

### Use this instead

Rename one of the colliding fields or preserve a nested namespace.

```rust,ignore
#[derive(serde::Serialize, serde::Deserialize)]
struct Envelope { envelope_id: u64, details: Details }
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

- [`rlib::serde_duplicate_serialized_names`](../serde_duplicate_serialized_names/README.md) — Finds collisions within one directly declared container.
