# `rlib::strum_manual_variant_names`

## Summary

Finds static string tables that repeat every enum variant's standard Strum name in declaration order.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::strum` |
| Cargo feature | `strum` |
| Purpose | Style |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds static string tables that repeat every enum variant's standard Strum name in declaration
order.

## Why this matters

The table must be updated whenever variants or naming attributes change. `VariantNames` derives the
same ordered names from the enum.

## Examples

### Triggers the lint

```rust,ignore
enum Stage { Planned, Active }
const STAGE_NAMES: &[&str] = &["Planned", "Active"];
```

### Use this instead

Derive `strum::VariantNames` and use `Stage::VARIANTS`. Review public table types before migrating.

```rust,ignore
#[derive(strum::VariantNames)]
enum Stage { Planned, Complete }
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when your project deliberately follows a different style.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
