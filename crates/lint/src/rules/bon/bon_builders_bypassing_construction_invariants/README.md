# bon_builders_bypassing_construction_invariants

## What it does

Finds Bon-derived structs with restricted fields and an inherent fallible constructor that builds the
same type.

## Why is this bad?

The derived builder assembles fields directly, allowing callers to avoid validation and cross-field
consistency enforced by the checked constructor.

## Example

```rust,ignore
#[derive(bon::Builder)]
pub struct DateRange { start: Date, end: Date }

impl DateRange {
    pub fn new(start: Date, end: Date) -> Result<Self, ReversedRange> { /* validate */ }
}
```

## Use instead

Generate the builder from the invariant-owning constructor.

```rust,ignore
#[bon]
impl DateRange {
    #[builder]
    pub fn new(start: Date, end: Date) -> Result<Self, ReversedRange> { /* validate */ }
}
```
