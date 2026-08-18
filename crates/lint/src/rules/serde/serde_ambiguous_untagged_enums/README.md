# `rlib::serde_ambiguous_untagged_enums`

## Summary

Finds pairs of named-field variants in a derived `#[serde(untagged)]` enum for which a concrete input object can deserialize as either variant.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::serde` |
| Cargo feature | `serde` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds pairs of named-field variants in a derived `#[serde(untagged)]` enum for which a concrete input
object can deserialize as either variant.

## Why this matters

Serde selects the first matching untagged variant. Overlapping shapes therefore make declaration
order part of the data format and can silently reinterpret input after a reorder or schema change.

## Examples

### Triggers the lint

```rust,ignore
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum Value {
    Detailed { id: u64, #[serde(default)] name: String },
    Minimal { id: u64 },
}
```

### Use this instead

Use an explicit tag or make the accepted shapes structurally disjoint.

```rust,ignore
#[derive(serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Value {
    Detailed { id: u64, name: String },
    Minimal { id: u64 },
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
