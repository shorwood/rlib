# `rlib::derive_more_operator_derives_bypassing_invariants`

## Summary

Finds derive_more operator, assignment, `Sum`, and `Product` derives on restricted-field types that also expose an inherent fallible constructor.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::derive_more` |
| Cargo feature | `derive_more` |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds derive_more operator, assignment, `Sum`, and `Product` derives on restricted-field types that
also expose an inherent fallible constructor.

## Why this matters

Component-wise operations reconstruct the type without passing through its checked construction
boundary. Bounded, normalized, nonzero, or cross-field state can therefore become invalid.

## Examples

### Triggers the lint

```rust,ignore
#[derive(derive_more::Add, derive_more::Sum)]
struct Percentage(u8);

impl Percentage {
    fn new(value: u8) -> Result<Self, OutOfRange> {
        (value <= 100).then_some(Self(value)).ok_or(OutOfRange)
    }
}
```

### Use this instead

Remove the field-based derives and implement only operations that preserve or revalidate the type's
invariants.

```rust,ignore
impl std::ops::Add for Percentage {
    type Output = Result<Self, OutOfRange>;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.0.checked_add(rhs.0).ok_or(OutOfRange)?)
    }
}
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

- [`rlib::derive_more_derived_constructors_bypassing_invariants`](../derive_more_derived_constructors_bypassing_invariants/README.md) — Covers generated constructors that bypass validation.
- [`rlib::derive_more_derived_conversions_bypassing_invariants`](../derive_more_derived_conversions_bypassing_invariants/README.md) — Covers generated conversions that bypass validation.
