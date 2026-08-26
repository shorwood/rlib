# `rlib::sqlx_panicking_row_access`

## Summary

Finds SQLx row accessors that panic when a column is absent or cannot be decoded.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::sqlx` |
| Cargo feature | `sqlx` |
| Purpose | Safety |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Resolved `Row::get` and `Row::column` calls in authored code.

## Why this matters

Database schema drift and decoding failures are runtime errors, not process invariants that justify a panic.

## Examples

### Triggers the lint

```rust,ignore
let id: i64 = row.get("id");
```

### Use this instead

```rust,ignore
let id: i64 = row.try_get("id")?;
```

## What it skips

Fallible accessors and non-SQLx methods with the same names are not reported.

## When to turn it off

Turn this lint off for a row whose schema is fixed and validated at the same boundary.

## Settings

This lint has no behavior-specific settings.

## Known limitations

The lint does not infer whether an earlier schema check makes a panicking access unreachable.

## Related lints

See `rlib::sqlx_unchecked_row_decoding`.
