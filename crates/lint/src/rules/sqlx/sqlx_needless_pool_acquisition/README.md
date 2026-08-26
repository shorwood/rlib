# `rlib::sqlx_needless_pool_acquisition`

## Summary

Finds pool connections acquired solely to execute one SQLx query.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::sqlx` |
| Cargo feature | `sqlx` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

A connection obtained from an SQLx `Pool` and referenced exactly once as a query executor.

## Why this matters

Pools already implement SQLx's executor interface, so the local connection adds lifecycle state without changing ownership.

## Examples

### Triggers the lint

```rust,ignore
let mut connection = pool.acquire().await?;
sqlx::query("DELETE FROM jobs").execute(&mut *connection).await?;
```

### Use this instead

```rust,ignore
sqlx::query("DELETE FROM jobs").execute(&pool).await?;
```

## What it skips

Connections used more than once or for operations other than one query execution are not reported.

## When to turn it off

Turn this lint off when explicit acquisition is part of measured connection timing or instrumentation.

## Settings

This lint has no behavior-specific settings.

## Known limitations

The use count is local to one function and does not follow a connection moved into another function.

## Related lints

See `rlib::sqlx_queries_in_loops`.
