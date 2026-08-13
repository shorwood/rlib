# derive_more_operator_derives_bypassing_invariants

## What it does

Finds derive_more operator, assignment, `Sum`, and `Product` derives on restricted-field types that
also expose an inherent fallible constructor.

## Why is this bad?

Component-wise operations reconstruct the type without passing through its checked construction
boundary. Bounded, normalized, nonzero, or cross-field state can therefore become invalid.

## Example

```rust,ignore
#[derive(derive_more::Add, derive_more::Sum)]
struct Percentage(u8);

impl Percentage {
    fn new(value: u8) -> Result<Self, OutOfRange> {
        (value <= 100).then_some(Self(value)).ok_or(OutOfRange)
    }
}
```

## Use instead

Remove the structural derives and implement only operations that preserve or revalidate the type's
invariants.

```rust,ignore
impl std::ops::Add for Percentage {
    type Output = Result<Self, OutOfRange>;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.0.checked_add(rhs.0).ok_or(OutOfRange)?)
    }
}
```
