# serde_deserialization_bypassing_invariants

## What it does

Finds derived `Deserialize` on restricted-field types that also expose an inherent fallible direct
constructor.

## Why is this bad?

Serde derive constructs fields directly. It can therefore create state that the type's checked
constructor rejects or normally normalizes.

## Example

```rust,ignore
#[derive(serde::Deserialize)]
struct Percentage(u8);

impl Percentage {
    fn new(value: u8) -> Result<Self, OutOfRange> { /* require <= 100 */ }
}
```

## Use instead

Deserialize through a validated conversion or a separate wire representation.

```rust,ignore
#[derive(serde::Deserialize)]
#[serde(try_from = "u8")]
struct Percentage(u8);
```
