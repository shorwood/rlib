# `rlib::ad_hoc_error_interfaces`

## Summary

Finds hand-written local structs and enums used as the concrete error argument of an active standard `Result` when they expose messages or causes through project-specific conventions.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds hand-written local structs and enums used as the concrete error argument of an active
standard `Result` when they expose messages or causes through project-specific conventions.
`Result` use or an error-like name alone is never sufficient: the lint requires a hand-written
textual field or accessor, standard formatter, or direct causal accessor.

Presentation evidence requires `Display`. Types named `*Error` or `*Failure`, and types with
a proven accessor exposing another `Error`, additionally require `Debug` and
`std::error::Error`. Causal evidence also requires `Error::source`; a nested field alone and
a collection of peer failures do not establish one predecessor. Handwritten and
derive-generated implementations both provide the standard behavior.

Lifetime parameters are supported, while type or const parameters, generated declarations,
serialized data, and known diagnostic, issue, rejection, response, report, or violation
families are preserved.

## Why this matters

Private message and cause conventions work for one call chain but cannot participate in
generic error bounds, reporting, context attachment, or standard source traversal.

## Examples

### Triggers the lint

```rust
struct ParseFailure { message: String }
fn parse() -> Result<(), ParseFailure> {
    Err(ParseFailure { message: "invalid".to_owned() })
}
```

### Use this instead

Prefer `Debug`, `Display`, and `std::error::Error` implementations on `ParseFailure`, using
`Error::source` only when one real causal predecessor exists.

No automatic rewrite is offered because presentation and causality require domain judgment.
Analysis is limited to the current compilation; active tests count while disabled configurations do not.

```rust
#[derive(Debug)]
struct ParseFailure { message: String }

impl std::fmt::Display for ParseFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ParseFailure {}
```

## What it skips

Causal evidence also requires `Error::source`; a nested field alone and a collection of peer failures do not establish one predecessor.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
