# `rlib::sqlx_queries_in_loops`

## Summary

Finds SQLx queries executed once per loop iteration.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::sqlx` |
| Cargo feature | `sqlx` |
| Purpose | Safety |
| Default level | `warn` |
| Fix | Manual |

## What it catches

SQLx fetch and execute operations nested in `for`, `while`, or `loop` bodies.

## Why this matters

Per-item database round trips multiply latency and can exhaust a connection pool under load.

## Examples

### Triggers the lint

```rust,ignore
for id in ids { sqlx::query("DELETE FROM jobs WHERE id = ?").bind(id).execute(&pool).await?; }
```

### Use this instead

```rust,ignore
// Bind the identifiers to one set-based statement or use QueryBuilder::push_values.
```

## What it skips

Query construction inside a loop is allowed when execution occurs once outside the loop.

## When to turn it off

Turn this lint off when the database operation must be sequential and batching is unsupported.

## Settings

This lint has no behavior-specific settings.

## Known limitations

The lint reports syntactic loop nesting and does not estimate iteration counts.

## Related lints

See `rlib::sqlx_dynamic_queries_cached_persistently`.
