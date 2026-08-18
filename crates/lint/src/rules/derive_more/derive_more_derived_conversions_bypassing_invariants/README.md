# `rlib::derive_more_derived_conversions_bypassing_invariants`

## Summary

Finds `derive_more::From` construction paths that bypass a type's fallible constructor.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::derive_more` |
| Cargo feature | `derive_more` |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds `derive_more::From` construction paths that bypass a type's fallible constructor. Repr-based
`derive_more::TryFrom` enum conversion is excluded because it does not populate restricted fields.

## Why this matters

A field-by-field conversion can populate fields that are less visible than their owning type even though
the type otherwise rejects invalid inputs through a checked construction boundary.

## Examples

### Triggers the lint

```rust,ignore
#[derive(derive_more::From)]
pub struct EmailAddress(String);

impl EmailAddress {
    fn parse(value: String) -> Result<Self, InvalidEmail> { /* validates */ }
}
```

### Use this instead

Remove the field-based construction derive and keep conversion behind `TryFrom` or a named checked
constructor that performs the invariant work.

```rust,ignore
impl TryFrom<String> for EmailAddress {
    type Error = InvalidEmail;
    fn try_from(value: String) -> Result<Self, Self::Error> { /* validates */ }
}
```

## What it skips

Repr-based `derive_more::TryFrom` enum conversion is excluded because it does not populate restricted fields.

## When to turn it off

Turn this lint off when the reported API is deliberate and callers depend on it.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

- [`rlib::derive_more_derived_constructors_bypassing_invariants`](../derive_more_derived_constructors_bypassing_invariants/README.md) — Covers generated constructors that bypass the same validation.
- [`rlib::derive_more_operator_derives_bypassing_invariants`](../derive_more_operator_derives_bypassing_invariants/README.md) — Covers generated operators that can create invalid values.
