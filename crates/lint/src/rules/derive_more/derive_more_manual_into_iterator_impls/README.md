# derive_more_manual_into_iterator_impls

## What it does

Finds owned, shared, and mutable `IntoIterator` implementations that delegate directly to the sole
field of a wrapper.

## Why is this bad?

Transparent iteration plumbing duplicates a structural contract and scatters the receiver family
across hand-written implementations.

## Example

```rust,ignore
impl IntoIterator for SystemList {
    type Item = System;
    type IntoIter = std::vec::IntoIter<System>;
    fn into_iter(self) -> Self::IntoIter { self.0.into_iter() }
}
```

## Use instead

Declare the required receiver forms together.

```rust,ignore
#[derive(derive_more::IntoIterator)]
#[into_iterator(owned, ref)]
struct SystemList(Vec<System>);
```
