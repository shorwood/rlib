# ad_hoc_formatting

## What it does

Finds hand-written inherent methods and free functions that derive one ordinary textual
representation of a local type, return `String` or `Cow<str>`, and use neutral names such as
`display`, `format`, `render_text`, or `to_text`. Helpers are grouped by represented type so
one missing contract or ambiguous family produces one diagnostic.

Methods must take only `&self`; free functions must take exactly one shared reference.
Lifetime parameters are accepted, while type and const parameters, unsafe or foreign
functions, async functions, trait methods, and generated declarations are ignored. Existing
`Display` implementations suppress the lint unless a helper provably delegates to them.
Explicit formats, configured rendering, borrowed `as_str` accessors, secret-bearing types,
and Clippy's exact inherent `to_string` pattern remain outside this lint.

## Why is this bad?

A project-specific canonical formatter hides ordinary presentation from formatting macros,
logging, error reporting, generic bounds, and allocation-aware writers. Several neutral
helpers also make canonical ownership ambiguous and can drift apart.

## Example

```rust
struct UserId(u64);
impl UserId {
    fn to_text(&self) -> String { format!("user-{}", self.0) }
}
```

## Use instead


Prefer one standard presentation contract:

```rust
use std::fmt::{self, Display, Formatter};
struct UserId(u64);
impl Display for UserId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "user-{}", self.0)
    }
}
```

No automatic rewrite is offered because choosing the canonical representation is behavioral.
Analysis is compilation-local; inactive configurations and downstream code are not visible.
