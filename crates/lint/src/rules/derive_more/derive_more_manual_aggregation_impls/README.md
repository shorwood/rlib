# derive_more_manual_aggregation_impls

## What it does

Finds owned `Sum` and `Product` implementations on single-field newtypes that map the sole field and
delegate directly to the field's corresponding aggregation.

## Why is this bad?

Mechanical aggregation code obscures meaningful policies such as checked arithmetic, early
termination, normalization, or a nonstandard empty-input identity.

## Example

```rust,ignore
impl std::iter::Sum for Total {
    fn sum<I: Iterator<Item = Self>>(values: I) -> Self {
        Self(values.map(|value| value.0).sum())
    }
}
```

## Use instead

```rust,ignore
#[derive(derive_more::Sum)]
struct Total(u64);
```
