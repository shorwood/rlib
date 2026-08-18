# `rlib::derive_more_manual_operator_impls`

## Summary

Finds standard unary, binary, and assignment operator implementations on single-field newtypes that perform only the same-field operation exactly as derive_more does.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::derive_more` |
| Cargo feature | `derive_more` |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds standard unary, binary, and assignment operator implementations on single-field newtypes that
perform only the same-field operation exactly as derive_more does.

## Why this matters

Mechanical operator forwarding adds code without documenting any domain-specific arithmetic policy.
Keeping only exceptional implementations makes validation, normalization, and unit conversion visible.

## Examples

### Triggers the lint

```rust,ignore
impl std::ops::Add for Offset {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}
```

### Use this instead

```rust,ignore
#[derive(derive_more::Add)]
struct Offset(i64);
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
