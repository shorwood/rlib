# derive_more_derived_constructors_bypassing_invariants

## What it does

Finds `derive_more::Constructor` uses that expose raw field assembly around a constructor with an
actual failure path. A `Result` return type that only ever constructs `Ok` is not treated as
validation evidence.

## Why is this bad?

The generated public `new` method can construct restricted fields without passing through the
type's checked invariant boundary.

## Example

```rust,ignore
#[derive(derive_more::Constructor)]
pub struct Percentage(u8);

impl Percentage {
    fn checked(value: u8) -> Result<Self, OutOfRange> { /* validates */ }
}
```

## Use instead

Remove `Constructor` and keep construction behind the fallible API. If unchecked construction is
intentional, represent it with a separate transparent type.

```rust,ignore
pub struct Percentage(u8);

impl Percentage {
    fn checked(value: u8) -> Result<Self, OutOfRange> { /* validates */ }
}
```
