# `rlib::derive_more_derived_constructors_bypassing_invariants`

## Summary

Finds `derive_more::Constructor` uses that expose raw field assembly around a constructor with an actual failure path.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::derive_more` |
| Cargo feature | `derive_more` |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds `derive_more::Constructor` uses that expose raw field assembly around a constructor with an
actual failure path. A `Result` return type that only ever constructs `Ok` is not treated as
validation evidence.

## Why this matters

The generated public `new` method can construct restricted fields without passing through the
type's checked invariant boundary.

## Examples

### Triggers the lint

```rust,ignore
#[derive(derive_more::Constructor)]
pub struct Percentage(u8);

impl Percentage {
    fn checked(value: u8) -> Result<Self, OutOfRange> { /* validates */ }
}
```

### Use this instead

Remove `Constructor` and keep construction behind the fallible API. If unchecked construction is
intentional, represent it with a separate transparent type.

```rust,ignore
pub struct Percentage(u8);

impl Percentage {
    fn checked(value: u8) -> Result<Self, OutOfRange> { /* validates */ }
}
```

## What it skips

A `Result` return type that only ever constructs `Ok` is not treated as validation evidence.

## When to turn it off

Turn this lint off when the reported API is deliberate and callers depend on it.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

- [`rlib::derive_more_derived_conversions_bypassing_invariants`](../derive_more_derived_conversions_bypassing_invariants/README.md) — Covers generated conversions that bypass the same validation.
- [`rlib::derive_more_operator_derives_bypassing_invariants`](../derive_more_operator_derives_bypassing_invariants/README.md) — Covers generated operators that can create invalid values.
