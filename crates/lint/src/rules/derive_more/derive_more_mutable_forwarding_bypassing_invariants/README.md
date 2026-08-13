# derive_more_mutable_forwarding_bypassing_invariants

## What it does

Finds derived `AsMut`, `DerefMut`, or `IndexMut` access that exposes restricted storage belonging to
a type with a fallible constructor.

## Why is this bad?

Unrestricted mutable access lets callers create states that the type's checked construction
boundary rejects.

## Example

```rust,ignore
#[derive(derive_more::Deref, derive_more::DerefMut)]
struct SortedValues(Vec<u32>);

impl SortedValues {
    fn checked(values: Vec<u32>) -> Result<Self, Unsorted> { /* validates */ }
}
```

## Use instead

Remove the mutable forwarding derive and expose mutation methods that preserve or revalidate the
invariant.

```rust,ignore
impl SortedValues {
    fn insert(&mut self, value: u32) { /* preserve ordering */ }
}
```
