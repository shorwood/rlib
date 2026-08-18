# `rlib::strum_manual_enum_iteration`

## Summary

Finds hand-written functions, methods, simple `IntoIterator` implementations, and iterator adapters that yield every unit enum variant exactly once in declaration order when Strum is a direct dependency.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::strum` |
| Cargo feature | `strum` |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds hand-written functions, methods, simple `IntoIterator` implementations, and iterator adapters
that yield every unit enum variant exactly once in declaration order when Strum is a direct
dependency.

## Why this matters

Manual enumeration duplicates the enum declaration and can silently become stale when a variant is
added. `EnumIter` keeps the enumeration protocol attached to the enum that owns it.

## Examples

### Triggers the lint

```rust,ignore
fn stages() -> impl Iterator<Item = Stage> {
    [Stage::Planned, Stage::Complete].into_iter()
}
```

### Use this instead

```rust,ignore
#[derive(strum::EnumIter)]
enum Stage { Planned, Complete }
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when the reported API is deliberate and callers depend on it.

## Settings

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `enum-variant-collection-provider` | string | not set | Chooses whether iteration or a static array should replace the hand-written collection. Values: `strum_enum_iter`, `strum_variant_array`. |

## Known limitations

No known implementation limitations.

## Related lints

None.
