# `rlib::derive_more_manual_equality_impls`

## Summary

Finds field-by-field `PartialEq` implementations, with an optional marker `Eq` implementation, that compare corresponding fields and nothing else.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::derive_more` |
| Cargo feature | `derive_more` |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds field-by-field `PartialEq` implementations, with an optional marker `Eq` implementation, that
compare corresponding fields and nothing else.

## Why this matters

Hand-written component equality hides a declarative field-selection policy and can drift when the
type changes.

## Examples

### Triggers the lint

```rust,ignore
impl PartialEq for Record {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.revision == other.revision
    }
}
```

### Use this instead

Declare field-by-field equality and explicitly skip any representation-only fields.

```rust,ignore
#[derive(derive_more::PartialEq, derive_more::Eq)]
struct Record { id: Id, revision: u64 }
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
