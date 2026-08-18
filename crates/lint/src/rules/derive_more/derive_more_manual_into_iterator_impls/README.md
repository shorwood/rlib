# `rlib::derive_more_manual_into_iterator_impls`

## Summary

Finds owned, shared, and mutable `IntoIterator` implementations that delegate directly to the sole field of a wrapper.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::derive_more` |
| Cargo feature | `derive_more` |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds owned, shared, and mutable `IntoIterator` implementations that delegate directly to the sole
field of a wrapper.

## Why this matters

Transparent iteration plumbing duplicates a field-based behavior and scatters the receiver family
across hand-written implementations.

## Examples

### Triggers the lint

```rust,ignore
impl IntoIterator for SystemList {
    type Item = System;
    type IntoIter = std::vec::IntoIter<System>;
    fn into_iter(self) -> Self::IntoIter { self.0.into_iter() }
}
```

### Use this instead

Declare the required receiver forms together.

```rust,ignore
#[derive(derive_more::IntoIterator)]
#[into_iterator(owned, ref)]
struct SystemList(Vec<System>);
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
