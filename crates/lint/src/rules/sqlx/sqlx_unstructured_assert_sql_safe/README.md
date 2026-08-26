# `rlib::sqlx_unstructured_assert_sql_safe`

## Summary

Finds primitive strings passed to SQLx's `assert_sql_safe` boundary.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::sqlx` |
| Cargo feature | `sqlx` |
| Purpose | Safety |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Calls to `assert_sql_safe` whose value is a string, string slice, or other primitive carrier.

## Why this matters

The method bypasses SQLx's ordinary display-safety boundary, so an unconstrained string can become executable SQL syntax.

## Examples

### Triggers the lint

```rust,ignore
builder.push(order).assert_sql_safe();
```

### Use this instead

```rust,ignore
enum Sort { Name, CreatedAt }
// Map each variant to a fixed SQL token at the query boundary.
```

## What it skips

Values carried by a dedicated non-primitive SQL fragment type are not reported.

## When to turn it off

Turn this lint off when the value is structurally constrained by an API the compiler cannot see.

## Settings

This lint has no behavior-specific settings.

## Known limitations

The lint uses the value's resolved Rust type and cannot prove invariants enforced only by runtime validation.

## Related lints

See `rlib::sqlx_unstructured_query_builder_fragments`.
