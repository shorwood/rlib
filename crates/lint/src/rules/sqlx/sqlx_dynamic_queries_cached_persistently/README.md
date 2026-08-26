# `rlib::sqlx_dynamic_queries_cached_persistently`

## Summary

Finds varying `QueryBuilder` statement shapes that retain SQLx's persistent prepared-statement caching.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::sqlx` |
| Cargo feature | `sqlx` |
| Purpose | Safety |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Queries built with `push_values` or `push_tuples` and executed without `.persistent(false)`.

## Why this matters

Each runtime cardinality can create another prepared statement on every connection and displace useful cached statements.

## Examples

### Triggers the lint

```rust,ignore
builder.push_values(rows, |mut values, row| { values.push_bind(row); });
builder.build().execute(&pool).await?;
```

### Use this instead

```rust,ignore
builder.build().persistent(false).execute(&pool).await?;
```

## What it skips

Fixed-shape builders and query chains already containing `.persistent(false)` are not reported.

## When to turn it off

Turn this lint off when the set of generated statement shapes is deliberately small and measured.

## Settings

This lint has no behavior-specific settings.

## Known limitations

Only SQLx builder operations that directly express varying bind cardinality are recognized.

## Related lints

See `rlib::sqlx_queries_in_loops`.
