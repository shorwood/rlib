# `rlib::sqlx_unchecked_row_decoding`

## Summary

Finds SQLx row accessors that skip type compatibility checks.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::sqlx` |
| Cargo feature | `sqlx` |
| Purpose | Correctness |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Resolved `Row::get_unchecked` and `Row::try_get_unchecked` calls.

## Why this matters

Unchecked access can reinterpret a schema mismatch as an unrelated Rust value or defer the decoding failure.

## Examples

### Triggers the lint

```rust,ignore
let name: String = row.try_get_unchecked("name")?;
```

### Use this instead

```rust,ignore
let name: String = row.try_get("name")?;
```

## What it skips

Checked SQLx row accessors and non-SQLx methods with the same names are not reported.

## When to turn it off

Turn this lint off when an external schema contract proves the unchecked Rust type mapping.

## Settings

This lint has no behavior-specific settings.

## Known limitations

The lint resolves the SQLx `Row` trait but cannot validate an external schema contract.

## Related lints

See `rlib::sqlx_panicking_row_access` and `rlib::sqlx_manual_row_mapping`.
