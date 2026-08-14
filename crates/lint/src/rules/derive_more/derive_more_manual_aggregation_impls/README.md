# derive_more_manual_aggregation_impls

## What it does

Finds owned `Sum<Self>` and `Product<Self>` implementations on tuple, named, and generic
single-field newtypes that map the sole field and delegate directly to the corresponding
aggregation. Borrowed-item aggregation is excluded because the derive would not replace it.

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
