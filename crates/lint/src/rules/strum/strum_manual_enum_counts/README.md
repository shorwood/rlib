# `rlib::strum_manual_enum_counts`

## Summary

Finds inherent constants and zero-argument methods whose integer value exactly repeats an enum's active variant count when Strum's `EnumCount` derive is directly available.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::strum` |
| Cargo feature | `strum` |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds inherent constants and zero-argument methods whose integer value exactly repeats an enum's
active variant count when Strum's `EnumCount` derive is directly available.

## Why this matters

A literal total is disconnected from the enum declaration and can become stale after variants are
added or removed.

## Examples

### Triggers the lint

```rust,ignore
impl Stage {
    const COUNT: usize = 2;
}
```

### Use this instead

```rust,ignore
#[derive(strum::EnumCount)]
enum Stage { Planned, Complete }
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when the reported API is deliberate and callers depend on it.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
