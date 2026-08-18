# `rlib::bare_tuple_types`

## Summary

Finds non-unit tuple types used as the root of an explicit type annotation.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds non-unit tuple types used as the root of an explicit type annotation. This includes
fields, parameters, return types, aliases, associated types, locals, and closure
annotations. Unit tuples and the compiler's internal tuple encoding for `Fn`, `FnMut`, and
`FnOnce` argument lists are excluded.

## Why this matters

A tuple records positions but not their meaning. Readers must recover each component's role
from surrounding code, and later additions or reordered elements are easy to misuse. A
concept-specific record also gives agents a names to reuse instead of propagating an
anonymous unnamed shape.

For example, this return type leaves both values unnamed:

## Examples

### Triggers the lint

```rust
fn partition() -> (Accepted, Rejected) {
    todo!()
}
```

### Use this instead

Introduce a record whose type and fields explain the result:

```rust
struct Partition {
    accepted: Accepted,
    rejected: Rejected,
}

fn partition() -> Partition {
    todo!()
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

- [`rlib::nested_tuple_types`](../nested_tuple_types/README.md) — Covers tuple types nested inside another type.
- [`rlib::positional_aggregate_fields`](../positional_aggregate_fields/README.md) — Covers unnamed fields in structs and enum variants.
