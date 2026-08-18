# `rlib::serde_manual_deserialize_impls`

## Summary

Finds `Deserialize` implementations on single-field newtypes that decode the field unchanged and immediately construct `Self`.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::serde` |
| Cargo feature | `serde` |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds `Deserialize` implementations on single-field newtypes that decode the field unchanged and
immediately construct `Self`.

## Why this matters

Transparent decoding boilerplate obscures the manual visitors that intentionally validate,
normalize, accept legacy forms, or provide richer errors.

## Examples

### Triggers the lint

```rust,ignore
impl<'de> serde::Deserialize<'de> for UserId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: serde::Deserializer<'de> {
        Ok(Self(u64::deserialize(deserializer)?))
    }
}
```

### Use this instead

```rust,ignore
#[derive(serde::Deserialize)]
#[serde(transparent)]
struct UserId(u64);
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
