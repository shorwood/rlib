# `rlib::bon_escaping_incomplete_builders`

## Summary

Finds generated Bon builders returned from private functions or stored in hand-written fields instead of being completed within a focused construction expression.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::bon` |
| Cargo feature | `bon` |
| Purpose | Safety |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds generated Bon builders returned from private functions or stored in hand-written fields
instead of being completed within a focused construction expression. Nested containers,
tuples, aliases, references, and pointers do not hide the generated builder state.

## Why this matters

Escaped generated builder state obscures which scope owns remaining required members and couples unrelated code to
the order and representation of staged construction.

## Examples

### Triggers the lint

```rust,ignore
fn base_request() -> RequestBuilder {
    Request::builder()
}
```

### Use this instead

Complete construction locally, or define a named domain state when partial application is an
intentional part of the API.

```rust,ignore
fn base_request() -> Request {
    Request::builder().host("example.test".to_owned()).build()
}
```

## What it skips

Nested containers, tuples, aliases, references, and pointers do not hide the generated builder state.

## When to turn it off

Turn this lint off only when the risk is handled elsewhere and documented.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
