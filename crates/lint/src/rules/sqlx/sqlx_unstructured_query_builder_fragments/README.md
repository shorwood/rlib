# `rlib::sqlx_unstructured_query_builder_fragments`

## Summary

Finds primitive values used as SQL syntax fragments in an SQLx `QueryBuilder`.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::sqlx` |
| Cargo feature | `sqlx` |
| Purpose | Safety |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Dynamic strings passed to `push` where the value controls SQL text rather than a bind parameter.

## Why this matters

Unstructured SQL fragments can mix user data with syntax and make the statement's allowed shapes impossible to audit.

## Examples

### Triggers the lint

```rust,ignore
builder.push(order_by);
```

### Use this instead

```rust,ignore
match order { Sort::Name => builder.push("name"), Sort::Date => builder.push("created_at") };
```

## What it skips

Static literals, `push_bind`, and dedicated fragment types are not reported.

## When to turn it off

Turn this lint off when a fragment type is safe but still represented as a primitive for an external compatibility boundary.

## Settings

This lint has no behavior-specific settings.

## Known limitations

The lint distinguishes fragment safety through Rust types and does not interpret string validation logic.

## Related lints

See `rlib::sqlx_unstructured_assert_sql_safe`.
