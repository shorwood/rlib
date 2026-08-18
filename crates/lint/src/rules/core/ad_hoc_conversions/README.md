# `rlib::ad_hoc_conversions`

## Summary

Finds unique, effect-free functions and inherent methods that consume exactly one concrete source value and construct one distinct local target.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds unique, effect-free functions and inherent methods that consume exactly one concrete
source value and construct one distinct local target. A direct target return is treated as
an infallible `From` opportunity; an exact `Result<Target, Error>` return is treated as a
fallible `TryFrom` opportunity.

The analysis follows the source through destructuring, local bindings, assignments,
branches, matches, explicit returns, and helper calls. It accepts concrete named, primitive,
tuple, array, slice, and reference sources while rejecting unresolved generic, opaque,
dynamic, and raw-pointer forms. Lifetimes do not split otherwise identical families.

## Why this matters

Ad hoc conversion functions hide standard capabilities from readers, generic code, IDEs,
and trait-driven APIs. They also invite several names for the same behavioral pair, making it
unclear which conversion is standard.

## Examples

### Triggers the lint

```rust
struct Record(String);
struct Account(String);

fn account_from_record(record: Record) -> Account {
    Account(record.0)
}
```

### Use this instead

Prefer declaring conversion ownership on the target:

```rust
struct Record(String);
struct Account(String);

impl From<Record> for Account {
    fn from(record: Record) -> Self {
        Self(record.0)
    }
}
```

Families with multiple candidates, existing `From` or `TryFrom` implementations, policy or
effect names, global static state, and known standard I/O effects are left alone.
Exact owned `&str -> Result<T, E>` parsers remain the responsibility of
`ad_hoc_string_parsers`. No automatic fix is offered because moving an API into a trait can
change visibility, coherence, error handling, and call syntax.

## What it skips

Lifetimes do not split otherwise identical families.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
