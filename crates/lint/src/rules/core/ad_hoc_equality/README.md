# ad_hoc_equality

## What it does

Finds free functions and inherent methods that compare two shared references to the same
local type, return `bool`, use both operands in a proven equality expression, and have an
unqualified equality name. Returned local aliases retain comparison provenance; discarded and
dormant comparisons do not. Existing direct `PartialEq` delegation remains valid.

## Why is this bad?

A project-specific equality protocol hides the relation from generic algorithms and can
drift away from `Hash`, collection behavior, or another helper. Multiple neutral helpers
also leave callers unable to tell which relation represents identity.

## Example

```rust
struct Slug(String);

fn slugs_equal(left: &Slug, right: &Slug) -> bool {
    left.0 == right.0
}
```

## Use instead

Implement `PartialEq` for the canonical relation. Keep business keys, compatibility,
case-folding, and security-sensitive comparisons explicitly named or represented by a
wrapper. `Eq` and `Hash` must only be added when their stronger laws are satisfied.

```rust
#[derive(PartialEq)]
struct Slug(String);
```
