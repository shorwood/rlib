# `rlib::constructor_like_free_functions`

## Summary

Finds same-module free functions whose declared return type is a local struct, enum, or union, directly or through nested `Option` and `Result` success containers, and whose body actually contains an expression that constructs that type.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | API design |
| Default level | `warn` |
| Fix | Automatic |

## What it catches

Finds same-module free functions whose declared return type is a local struct, enum, or
union, directly or through nested `Option` and `Result` success containers, and whose body
actually contains an expression that constructs that type. Functions already shaped like
instance methods are left to `method_like_free_functions`.

Construction must contribute to the returned value. This includes returned aliases and
adapter closures such as `Option::map`, but excludes discarded values and closure bodies
stored for later use.

A unique standard `&str -> Result<T, E>` parser is also left to
`ad_hoc_string_parsers`, which can recommend the standard `FromStr` trait. Ambiguous or
deliberately qualified parser families still receive this ownership warning when written
as free functions.

## Why this matters

Construction defines which inputs establish a type's invariants. Keeping that behavior as
a free function makes it harder to discover, separates it from sibling constructors, and
encourages vague names that only become meaningful through module context.

## Examples

### Triggers the lint

```rust
struct Session {
    token: String,
}

fn create_session(token: String) -> Session {
    Session { token }
}
```

### Use this instead

Put construction on the type and use the move to reconsider the behavioral name:

```rust
struct Session {
    token: String,
}

impl Session {
    fn authenticated(token: String) -> Self {
        Self { token }
    }
}
```

A machine-applicable fix is offered only for a private, attribute-free function immediately
following its nongeneric target declaration or contiguous inherent impl group. Every
reference must be editable in the same file, with no imports, macros, comments in the move
boundary, or associated-name collision. Other findings remain guidance because choosing an
ownership-preserving move is more important than forcing a mechanical rewrite.

## What it skips

This includes returned aliases and adapter closures such as `Option::map`, but excludes discarded values and closure bodies stored for later use.

## When to turn it off

Turn this lint off when the reported API is deliberate and callers depend on it.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
