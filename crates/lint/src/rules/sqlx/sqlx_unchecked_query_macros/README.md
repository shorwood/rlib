# `rlib::sqlx_unchecked_query_macros`

## Summary

Finds SQLx query macros that skip output type checking.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::sqlx` |
| Cargo feature | `sqlx` |
| Purpose | Correctness |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Calls to `query_unchecked!`, `query_as_unchecked!`, and their file-based forms.

## Why this matters

Unchecked macros validate SQL but allow the declared Rust output types to disagree with database columns.

## Examples

### Triggers the lint

```rust,ignore
let user = sqlx::query_as_unchecked!(User, "SELECT id FROM users");
```

### Use this instead

```rust,ignore
let user = sqlx::query_as!(User, "SELECT id FROM users");
```

## What it skips

Checked SQLx macros and runtime query functions are handled separately.

## When to turn it off

Turn this lint off only when SQLx cannot express a database-specific output type and the mapping is reviewed separately.

## Settings

This lint has no behavior-specific settings.

## Known limitations

The lint identifies SQLx macro definitions and does not inspect macros that wrap them.

## Related lints

See `rlib::sqlx_static_queries_without_compile_time_checks`.
