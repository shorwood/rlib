# `rlib::strum_manual_enum_string_parsers`

## Summary

Finds exhaustive unit-enum `FromStr` and inherent Option parsers reproducible by Strum's `EnumString` derive.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::strum` |
| Cargo feature | `strum` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds exhaustive unit-enum `FromStr` matches and inherent `parse(&str) -> Option<Self>` shims
reproducible by Strum's `EnumString` derive.

## Why this matters

The parser repeats variant names, aliases, and failure behavior that must remain synchronized with
the enum.

## Examples

### Triggers the lint

```rust,ignore
impl FromStr for Stage {
    type Err = strum::ParseError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value { "Planned" => Ok(Self::Planned), _ => Err(strum::ParseError::VariantNotFound) }
    }
}
```

### Use this instead

Select the configured parser provider and remove only the exactly equivalent implementation.

```rust,ignore
#[derive(strum::EnumString)]
enum Stage { Planned, Complete }
```

## What it skips

Skips normalization, guards, payload construction, and nonliteral keys. Rustdoc does not change
structural recognition; use an explicit lint control for a deliberate exception.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `enum-string-parsing-provider` | string | not set | Chooses the derive that should generate string parsing. Values: `strum_enum_string`, `derive_more_from_str`. |

## Known limitations

No known implementation limitations.

## Related lints

None.
