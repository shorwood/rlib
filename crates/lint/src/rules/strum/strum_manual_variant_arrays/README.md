# `rlib::strum_manual_variant_arrays`

## Summary

Finds exhaustive declaration-order arrays and slices of unit enum variants when Strum's `VariantArray` derive is directly available and explicitly selected for ambiguous collections.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::strum` |
| Cargo feature | `strum` |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds exhaustive declaration-order arrays and slices of unit enum variants when Strum's
`VariantArray` derive is directly available and explicitly selected for ambiguous collections.

## Why this matters

A hand-written variant array repeats information already owned by the enum and must be updated by hand
whenever the enum changes.

## Examples

### Triggers the lint

```rust,ignore
impl Stage {
    const ALL: [Self; 2] = [Self::Planned, Self::Complete];
}
```

### Use this instead

```rust,ignore
#[derive(strum::VariantArray)]
enum Stage { Planned, Complete }
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when the reported API is deliberate and callers depend on it.

## Settings

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `enum-variant-collection-provider` | string | not set | Chooses whether a static array or iteration should replace the hand-written collection. Values: `strum_variant_array`, `strum_enum_iter`. |

## Known limitations

No known implementation limitations.

## Related lints

None.
