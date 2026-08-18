# `rlib::derive_more_manual_conversion_impls`

## Summary

Finds hand-written `From` implementations that only construct or extract the sole field of a newtype.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::derive_more` |
| Cargo feature | `derive_more` |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds hand-written `From` implementations that only construct or extract the sole field of a newtype.

## Why this matters

Transparent conversion plumbing repeats a standard implementation while obscuring that no
validation, normalization, or policy is present.

## Examples

### Triggers the lint

```rust,ignore
impl From<Vec<System>> for SystemList {
    fn from(systems: Vec<System>) -> Self { Self(systems) }
}
```

### Use this instead

Express the field-based conversion at the type declaration.

```rust,ignore
#[derive(derive_more::From, derive_more::Into)]
struct SystemList(Vec<System>);
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
