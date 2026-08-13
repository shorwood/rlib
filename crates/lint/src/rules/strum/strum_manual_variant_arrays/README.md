# strum_manual_variant_arrays

## What it does

Finds exhaustive declaration-order arrays and slices of unit enum variants when Strum's
`VariantArray` derive is directly available and explicitly selected for ambiguous collections.

## Why is this bad?

A hand-written variant array repeats information already owned by the enum and must be updated by hand
whenever the enum changes.

## Example

```rust,ignore
impl Stage {
    const ALL: [Self; 2] = [Self::Planned, Self::Complete];
}
```

## Use instead

```rust,ignore
#[derive(strum::VariantArray)]
enum Stage { Planned, Complete }
```
