# constructor_like_free_functions

## What it does

Finds same-module free functions whose declared return type is a local struct, enum, or
union, directly or through nested `Option` and `Result` success containers, and whose body
actually contains an expression that constructs that type. Functions already shaped like
instance methods are left to `method_like_free_functions`.

A unique canonical `&str -> Result<T, E>` parser is also left to
`ad_hoc_string_parsers`, which can recommend the stronger `FromStr` contract. Ambiguous or
deliberately qualified parser families still receive this ownership warning when written
as free functions.

## Why is this bad?

Construction defines which inputs establish a type's invariants. Keeping that behavior as
a free function makes it harder to discover, separates it from sibling constructors, and
encourages vague names that only become meaningful through module context.

## Example

```rust
struct Session {
    token: String,
}

fn create_session(token: String) -> Session {
    Session { token }
}
```

## Use instead

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
