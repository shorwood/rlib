# `rlib::sqlx_query_builder_reused_without_reset`

## Summary

Finds an SQLx `QueryBuilder` reused after `build` without an intervening `reset`.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::sqlx` |
| Cargo feature | `sqlx` |
| Purpose | Correctness |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Further `push` or `build` operations on the same local builder after any `build` method.

## Why this matters

SQLx requires `reset` before builder reuse and otherwise panics at runtime.

## Examples

### Triggers the lint

```rust,ignore
let query = builder.build();
drop(query);
builder.push(" LIMIT 1");
```

### Use this instead

```rust,ignore
let query = builder.build();
drop(query);
builder.reset();
builder.push(" LIMIT 1");
```

## What it skips

Fresh builders and reuse after a resolved `reset` call are not reported.

## When to turn it off

Turn this lint off only if a wrapper performs the reset through behavior the compiler cannot resolve.

## Settings

This lint has no behavior-specific settings.

## Known limitations

The state analysis follows one local binding and is conservative around aliases and branches.

## Related lints

See `rlib::sqlx_dynamic_queries_cached_persistently`.
