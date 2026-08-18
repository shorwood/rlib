# `rlib::ad_hoc_equality`

## Summary

Finds free functions and inherent methods that compare two shared references to the same local type, return `bool`, use both operands in a proven equality expression, and have an unqualified equality name.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds free functions and inherent methods that compare two shared references to the same
local type, return `bool`, use both operands in a proven equality expression, and have an
unqualified equality name. Returned local aliases retain their link to the comparison; discarded and
dormant comparisons do not. Existing direct `PartialEq` delegation remains valid.

## Why this matters

A project-specific equality method hides the relation from generic algorithms and can
drift away from `Hash`, collection behavior, or another helper. Multiple neutral helpers
also leave callers unable to tell which relation represents identity.

## Examples

### Triggers the lint

```rust
struct Slug(String);

fn slugs_equal(left: &Slug, right: &Slug) -> bool {
    left.0 == right.0
}
```

### Use this instead

Implement `PartialEq` for the standard relation. Keep business keys, compatibility,
case-folding, and security-sensitive comparisons explicitly named or represented by a
wrapper. `Eq` and `Hash` must only be added when their stronger laws are satisfied.

```rust
#[derive(PartialEq)]
struct Slug(String);
```

## What it skips

Returned local aliases retain their link to the comparison; discarded and dormant comparisons do not.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

- [`rlib::ad_hoc_ordering`](../ad_hoc_ordering/README.md) — The matching rule for hand-written ordering helpers.
