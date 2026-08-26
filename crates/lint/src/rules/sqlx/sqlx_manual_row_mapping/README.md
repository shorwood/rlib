# `rlib::sqlx_manual_row_mapping`

## Summary

Finds structs populated by repeated extraction from one SQLx row.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::sqlx` |
| Cargo feature | `sqlx` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Struct expressions with at least two fields populated by `get` or `try_get` calls on one row.

## Why this matters

Mechanical field extraction duplicates schema mapping and makes decoding behavior inconsistent across fields.

## Examples

### Triggers the lint

```rust,ignore
User { id: row.try_get("id")?, name: row.try_get("name")? }
```

### Use this instead

```rust,ignore
#[derive(sqlx::FromRow)]
struct User { id: i64, name: String }
```

## What it skips

Single-field wrappers, computed fields, and structs assembled from more than one row are not reported.

## When to turn it off

Turn this lint off when field construction deliberately applies distinct domain conversions.

## Settings

This lint has no behavior-specific settings.

## Known limitations

The lint recognizes direct struct construction and not later assignments to an existing value.

## Related lints

See `rlib::sqlx_unchecked_row_decoding`.
