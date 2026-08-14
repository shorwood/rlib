# derive_more_inconsistent_derived_equality

## What it does

Finds `derive_more::PartialEq` struct and enum configurations that use an exact field-level `skip`
while a generated `Hash` or `Ord` implementation still includes that field.

## Why is this bad?

Values considered equal must hash identically, and total ordering must report `Equal` exactly when
equality does. Comparing fewer fields than those structural derives violates these laws and breaks
hash-map or ordered-set behavior.

## Example

```rust,ignore
#[derive(Hash, derive_more::PartialEq)]
struct Key {
    id: u64,
    #[partial_eq(skip)]
    revision: u64,
}
```

## Use instead

Use the same field set for equality, hashing, and ordering, or introduce a wrapper representing the
alternative identity.

```rust,ignore
#[derive(Hash, derive_more::PartialEq)]
struct Key { id: u64, revision: u64 }
```
