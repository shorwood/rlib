# `rlib::nested_tuple_types`

## Summary

Finds non-unit tuple types nested inside another explicit type, including references, slices, arrays, collections, options, results, and associated type bindings.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds non-unit tuple types nested inside another explicit type, including references,
slices, arrays, collections, options, results, and associated type bindings. Positional
arguments in `Fn`, `FnMut`, and `FnOnce` syntax are not tuple types; real tuples elsewhere in
the same annotation are still checked.

## Why this matters

Nesting an anonymous tuple makes an already tuple type harder to understand. The
surrounding container explains storage or ownership, but it cannot explain what each tuple
position represents. Repeating the shape also spreads an unnamed convention across APIs.

For example, neither component of each affected entry is named:

## Examples

### Triggers the lint

```rust
fn affected() -> &'static [(&'static Participant, NameTokens)] {
    todo!()
}
```

### Use this instead

Name the element concept and its roles before placing it in the container:

```rust
struct AffectedParticipant<'a> {
    participant: &'a Participant,
    name: NameTokens,
}

fn affected() -> &'static [AffectedParticipant<'static>] {
    todo!()
}
```

## What it skips

Positional arguments in `Fn`, `FnMut`, and `FnOnce` syntax are not tuple types; real tuples elsewhere in the same annotation are still checked.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

- [`rlib::bare_tuple_types`](../bare_tuple_types/README.md) — Covers tuples used as the entire declared type.
- [`rlib::positional_aggregate_fields`](../positional_aggregate_fields/README.md) — Covers unnamed fields in structs and enum variants.
