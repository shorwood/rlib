# `rlib::bon_builders_bypassing_construction_invariants`

## Summary

Finds Bon-derived structs with fields less visible than the struct itself and an inherent fallible constructor that builds the same type.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::bon` |
| Cargo feature | `bon` |
| Purpose | Safety |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds Bon-derived structs with fields less visible than the struct itself and an inherent
fallible constructor that builds the same type. A builder on a private type whose fields have
the same effective reach does not expand the ways to construct the type and remains valid.

## Why this matters

The derived builder assembles fields directly, allowing callers to avoid validation and cross-field
consistency enforced by the checked constructor.

## Examples

### Triggers the lint

```rust,ignore
#[derive(bon::Builder)]
pub struct DateRange { start: Date, end: Date }

impl DateRange {
    pub fn new(start: Date, end: Date) -> Result<Self, ReversedRange> { /* validate */ }
}
```

### Use this instead

Generate the builder from the invariant-owning constructor.

```rust,ignore
#[bon]
impl DateRange {
    #[builder]
    pub fn new(start: Date, end: Date) -> Result<Self, ReversedRange> { /* validate */ }
}
```

## What it skips

A builder on a private type whose fields have the same effective reach does not expand the ways to construct the type and remains valid.

## When to turn it off

Turn this lint off only when the risk is handled elsewhere and documented.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
