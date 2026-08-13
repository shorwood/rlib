# derive_more_derived_conversions_bypassing_invariants

## What it does

Finds `derive_more::From` or `derive_more::TryFrom` construction paths that bypass a type's fallible
constructor.

## Why is this bad?

Structural conversion can populate restricted state directly even though the type otherwise rejects
invalid inputs through a checked construction boundary.

## Example

```rust,ignore
#[derive(derive_more::From)]
struct EmailAddress(String);

impl EmailAddress {
    fn parse(value: String) -> Result<Self, InvalidEmail> { /* validates */ }
}
```

## Use instead

Remove the structural construction derive and keep conversion behind `TryFrom` or a named checked
constructor that performs the invariant work.

```rust,ignore
impl TryFrom<String> for EmailAddress {
    type Error = InvalidEmail;
    fn try_from(value: String) -> Result<Self, Self::Error> { /* validates */ }
}
```
