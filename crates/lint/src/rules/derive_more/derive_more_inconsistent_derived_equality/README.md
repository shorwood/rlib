# `rlib::derive_more_inconsistent_derived_equality`

## Summary

Finds `derive_more::PartialEq` struct and enum configurations that use an exact field-level `skip` while a generated `Hash` or `Ord` implementation still includes that field.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::derive_more` |
| Cargo feature | `derive_more` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds `derive_more::PartialEq` struct and enum configurations that use an exact field-level `skip`
while a generated `Hash` or `Ord` implementation still includes that field.

## Why this matters

Values considered equal must hash identically, and total ordering must report `Equal` exactly when
equality does. Comparing fewer fields than those field-based derives violates these laws and breaks
hash-map or ordered-set behavior.

## Examples

### Triggers the lint

```rust,ignore
#[derive(Hash, derive_more::PartialEq)]
struct Key {
    id: u64,
    #[partial_eq(skip)]
    revision: u64,
}
```

### Use this instead

Use the same field set for equality, hashing, and ordering, or introduce a wrapper representing the
alternative identity.

```rust,ignore
#[derive(Hash, derive_more::PartialEq)]
struct Key { id: u64, revision: u64 }
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
