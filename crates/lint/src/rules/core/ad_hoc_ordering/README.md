# `rlib::ad_hoc_ordering`

## Summary

Finds neutral free functions and inherent methods over two shared references to one local type.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds neutral free functions and inherent methods over two shared references to one local
type. A proven `Ordering` result is treated as an `Ord` opportunity; a proven
`Option<Ordering>` result is treated as a `PartialOrd` opportunity. Returned local aliases and
explicit returns retain their link to the comparison; discarded and dormant comparisons do not.

## Why this matters

Named standard comparators hide ordering from generic code and ordered collections while
allowing equality, sorting, and deduplication to evolve independently. Several unqualified
comparators make the ambiguity worse rather than establishing useful domain names.

## Examples

### Triggers the lint

```rust
use std::cmp::Ordering;
struct Slug(String);
fn compare_slugs(left: &Slug, right: &Slug) -> Ordering {
    left.0.cmp(&right.0)
}
```

### Use this instead

Contextual orderings such as display order, priority, or business-key order remain named.
Float-backed types are ignored unless the returned relation explicitly establishes total
ordering.

```rust
#[derive(Eq, Ord, PartialEq, PartialOrd)]
struct Slug(String);
```

## What it skips

Returned local aliases and explicit returns retain their link to the comparison; discarded and dormant comparisons do not.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

- [`rlib::ad_hoc_equality`](../ad_hoc_equality/README.md) — The matching rule for hand-written equality helpers.
