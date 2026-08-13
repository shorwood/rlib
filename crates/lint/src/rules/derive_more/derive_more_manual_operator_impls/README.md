# derive_more_manual_operator_impls

## What it does

Finds standard unary, binary, and assignment operator implementations on single-field newtypes that
perform only the same-field operation exactly as derive_more does.

## Why is this bad?

Mechanical operator forwarding adds code without documenting any domain-specific arithmetic policy.
Keeping only exceptional implementations makes validation, normalization, and unit conversion visible.

## Example

```rust,ignore
impl std::ops::Add for Offset {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}
```

## Use instead

```rust,ignore
#[derive(derive_more::Add)]
struct Offset(i64);
```
