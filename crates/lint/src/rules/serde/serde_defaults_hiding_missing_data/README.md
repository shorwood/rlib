# `rlib::serde_defaults_hiding_missing_data`

## Summary

Finds undocumented bare `#[serde(default)]` fields whose type is not `Option`.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::serde` |
| Cargo feature | `serde` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds undocumented bare `#[serde(default)]` fields whose type is not `Option`.

## Why this matters

A missing input silently becomes the type's generic `Default`, which can turn absence into a
meaningful quantity, identifier, flag, path, or timestamp without expressing schema policy.

## Examples

### Triggers the lint

```rust,ignore
#[derive(serde::Deserialize)]
struct Limits {
    #[serde(default)]
    maximum: u32,
}
```

### Use this instead

Use optionality or a named default that states the compatibility policy.

```rust,ignore
fn default_maximum() -> u32 { 100 }

struct Limits {
    #[serde(default = "default_maximum")]
    maximum: u32,
}
```

## What it skips

Finds undocumented bare `#[serde(default)]` fields whose type is not `Option`.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
