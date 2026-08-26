# `rlib::sqlx_transactions_implicitly_rolled_back`

## Summary

Finds SQLx transactions with no explicit `commit` or `rollback`.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::sqlx` |
| Cargo feature | `sqlx` |
| Purpose | Correctness |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Local transactions created with `begin` that are never consumed by `commit` or `rollback` in the function.

## Why this matters

Dropping a live transaction rolls it back implicitly and hides the function's database outcome in destructor behavior.

## Examples

### Triggers the lint

```rust,ignore
let mut transaction = pool.begin().await?;
update(&mut transaction).await?;
```

### Use this instead

```rust,ignore
let mut transaction = pool.begin().await?;
update(&mut transaction).await?;
transaction.commit().await?;
```

## What it skips

Transactions with an explicit resolved `commit` or `rollback` call are not reported.

## When to turn it off

Turn this lint off when an enclosing abstraction deliberately owns transaction finalization.

## Settings

This lint has no behavior-specific settings.

## Known limitations

The analysis proves the presence of explicit finalization but does not yet prove it occurs on every control-flow path.

## Related lints

See `rlib::sqlx_needless_pool_acquisition`.
