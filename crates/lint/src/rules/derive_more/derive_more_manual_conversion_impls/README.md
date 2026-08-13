# derive_more_manual_conversion_impls

## What it does

Finds authored `From` implementations that only construct or extract the sole field of a newtype.

## Why is this bad?

Transparent conversion plumbing repeats a standard implementation while obscuring that no
validation, normalization, or policy is present.

## Example

```rust,ignore
impl From<Vec<System>> for SystemList {
    fn from(systems: Vec<System>) -> Self { Self(systems) }
}
```

## Use instead

Express the structural conversion at the type declaration.

```rust,ignore
#[derive(derive_more::From, derive_more::Into)]
struct SystemList(Vec<System>);
```
