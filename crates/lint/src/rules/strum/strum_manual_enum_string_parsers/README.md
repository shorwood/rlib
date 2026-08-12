# strum_manual_enum_string_parsers

## What it does

Finds exhaustive unit-enum `FromStr` matches reproducible by Strum's `EnumString` derive.

## Why is this bad?

The parser repeats variant names, aliases, and failure behavior that must remain synchronized with
the enum.

## Example

```rust,ignore
impl FromStr for Stage {
    type Err = strum::ParseError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value { "Planned" => Ok(Self::Planned), _ => Err(strum::ParseError::VariantNotFound) }
    }
}
```

## Use instead

Select the configured parser provider and remove only the exactly equivalent implementation.

```rust,ignore
#[derive(strum::EnumString)]
enum Stage { Planned, Complete }
```
