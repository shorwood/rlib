# `rlib::derive_more_mutable_forwarding_bypassing_invariants`

## Summary

Finds derived `AsMut`, `DerefMut`, or `IndexMut` access that exposes restricted storage belonging to a type with a fallible constructor.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::derive_more` |
| Cargo feature | `derive_more` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds derived `AsMut`, `DerefMut`, or `IndexMut` access that exposes restricted storage belonging to
a type with a fallible constructor.

## Why this matters

Unrestricted mutable access lets callers create states that the type's checked construction
boundary rejects.

## Examples

### Triggers the lint

```rust,ignore
#[derive(derive_more::Deref, derive_more::DerefMut)]
struct SortedValues(Vec<u32>);

impl SortedValues {
    fn checked(values: Vec<u32>) -> Result<Self, Unsorted> { /* validates */ }
}
```

### Use this instead

Remove the mutable forwarding derive and expose mutation methods that preserve or revalidate the
invariant.

```rust,ignore
impl SortedValues {
    fn insert(&mut self, value: u32) { /* preserve ordering */ }
}
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
