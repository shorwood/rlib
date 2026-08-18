# `rlib::serde_deserialization_bypassing_invariants`

## Summary

Finds derived `Deserialize` on restricted-field types that also expose an inherent fallible direct constructor.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::serde` |
| Cargo feature | `serde` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds derived `Deserialize` on restricted-field types that also expose an inherent fallible direct
constructor.

## Why this matters

Serde derive constructs fields directly. It can therefore create state that the type's checked
constructor rejects or normally normalizes.

## Examples

### Triggers the lint

```rust,ignore
#[derive(serde::Deserialize)]
struct Percentage(u8);

impl Percentage {
    fn new(value: u8) -> Result<Self, OutOfRange> { /* require <= 100 */ }
}
```

### Use this instead

Deserialize through a validated conversion or a separate wire representation.

```rust,ignore
#[derive(serde::Deserialize)]
#[serde(try_from = "u8")]
struct Percentage(u8);
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
