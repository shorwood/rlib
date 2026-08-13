# derive_more_manual_from_str_impls

## What it does

Finds newtype `FromStr` implementations that parse the sole field and map it directly into the
wrapper.

## Why is this bad?

Transparent parser forwarding duplicates a structural contract while implying that validation,
normalization, aliases, or custom error policy might be present.

## Example

```rust,ignore
impl FromStr for Port {
    type Err = ParseIntError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value.parse().map(Self)
    }
}
```

## Use instead

Declare the exact newtype parser on the wrapper.

```rust,ignore
#[derive(derive_more::FromStr)]
struct Port(u16);
```
