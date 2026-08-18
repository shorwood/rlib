# `rlib::derive_more_manual_from_str_impls`

## Summary

Finds newtype `FromStr` implementations that parse the sole field and map it directly into the wrapper.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::derive_more` |
| Cargo feature | `derive_more` |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds newtype `FromStr` implementations that parse the sole field and map it directly into the
wrapper.

## Why this matters

Transparent parser forwarding duplicates a field-based behavior while implying that validation,
normalization, aliases, or custom error policy might be present.

## Examples

### Triggers the lint

```rust,ignore
impl FromStr for Port {
    type Err = ParseIntError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value.parse().map(Self)
    }
}
```

### Use this instead

Declare the exact newtype parser on the wrapper.

```rust,ignore
#[derive(derive_more::FromStr)]
struct Port(u16);
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when the reported API is deliberate and callers depend on it.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
