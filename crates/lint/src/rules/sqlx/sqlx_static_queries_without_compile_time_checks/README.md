# `rlib::sqlx_static_queries_without_compile_time_checks`

## Summary

Finds static SQL passed through SQLx's runtime query functions instead of checked macros.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::sqlx` |
| Cargo feature | `sqlx` |
| Purpose | Correctness |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Literal SQL supplied to `query`, `query_as`, `query_scalar`, and related runtime functions.

## Why this matters

Static statements can validate syntax, bind parameters, and row types during compilation instead of failing after deployment.

## Examples

### Triggers the lint

```rust,ignore
let users = sqlx::query("SELECT id FROM users");
```

### Use this instead

```rust,ignore
let users = sqlx::query!("SELECT id FROM users");
```

## What it skips

Runtime-computed SQL and `raw_sql` are not reported because they cannot use checked query macros directly.

## When to turn it off

Turn this lint off when deployment intentionally performs schema validation independently of Rust compilation.

## Settings

This lint has no behavior-specific settings.

## Known limitations

Only literals, trivial wrappers, and literal concatenation are classified as static.

## Related lints

See `rlib::sqlx_unchecked_query_macros`.
