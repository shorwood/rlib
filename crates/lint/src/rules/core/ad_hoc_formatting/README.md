# `rlib::ad_hoc_formatting`

## Summary

Finds hand-written inherent methods and free functions that derive one ordinary textual representation of a local type, return `String` or `Cow<str>`, and use neutral names such as `display`, `format`, `render_text`, or `to_text`.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds hand-written inherent methods and free functions that derive one ordinary textual
representation of a local type, return `String` or `Cow<str>`, and use neutral names such as
`display`, `format`, `render_text`, or `to_text`. Helpers are grouped by represented type so
one missing standard implementation or ambiguous family produces one warning.

Methods must take only `&self`; free functions must take exactly one shared reference.
Lifetime parameters are accepted, while type and const parameters, unsafe or foreign
functions, async functions, trait methods, and generated declarations are ignored. Existing
`Display` implementations suppress the lint unless a helper provably delegates to them.
Explicit formats, configured rendering, borrowed `as_str` accessors, secret-bearing types,
and Clippy's exact inherent `to_string` pattern remain outside this lint.

## Why this matters

A project-specific standard formatter hides ordinary presentation from formatting macros,
logging, error reporting, generic bounds, and allocation-aware writers. Several neutral
helpers also make standard ownership ambiguous and can drift apart.

## Examples

### Triggers the lint

```rust
struct UserId(u64);
impl UserId {
    fn to_text(&self) -> String { format!("user-{}", self.0) }
}
```

### Use this instead

Prefer one standard display rule:

```rust
use std::fmt::{self, Display, Formatter};
struct UserId(u64);
impl Display for UserId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "user-{}", self.0)
    }
}
```

No automatic rewrite is offered because choosing the standard representation is behavioral.
Analysis is limited to the current compilation; inactive configurations and downstream code are not visible.

## What it skips

Lifetime parameters are accepted, while type and const parameters, unsafe or foreign functions, async functions, trait methods, and generated declarations are ignored. Explicit formats, configured rendering, borrowed `as_str` accessors, secret-bearing types, and Clippy's exact inherent `to_string` pattern remain outside this lint.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
