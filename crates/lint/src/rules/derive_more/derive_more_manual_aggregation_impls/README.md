# `rlib::derive_more_manual_aggregation_impls`

## Summary

Finds owned `Sum<Self>` and `Product<Self>` implementations on tuple, named, and generic single-field newtypes that map the sole field and delegate directly to the corresponding aggregation.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::derive_more` |
| Cargo feature | `derive_more` |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds owned `Sum<Self>` and `Product<Self>` implementations on tuple, named, and generic
single-field newtypes that map the sole field and delegate directly to the corresponding
aggregation. Borrowed-item aggregation is excluded because the derive would not replace it.

## Why this matters

Mechanical aggregation code obscures meaningful policies such as checked arithmetic, early
termination, normalization, or a nonstandard empty-input identity.

## Examples

### Triggers the lint

```rust,ignore
impl std::iter::Sum for Total {
    fn sum<I: Iterator<Item = Self>>(values: I) -> Self {
        Self(values.map(|value| value.0).sum())
    }
}
```

### Use this instead

```rust,ignore
#[derive(derive_more::Sum)]
struct Total(u64);
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
