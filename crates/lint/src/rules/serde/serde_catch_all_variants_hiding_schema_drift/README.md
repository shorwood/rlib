# `rlib::serde_catch_all_variants_hiding_schema_drift`

## Summary

Finds `#[serde(other)]` variants in public enums with derived deserialization.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::serde` |
| Cargo feature | `serde` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds `#[serde(other)]` variants in public enums with derived deserialization.

## Why this matters

At an externally visible boundary, a catch-all turns new or misspelled wire variants into one
generic state. That hides schema drift, loses the unknown spelling, and can accidentally select
permissive behavior.

## Examples

### Triggers the lint

```rust,ignore
#[derive(serde::Deserialize)]
pub enum Permission { Read, Write, #[serde(other)] Unknown }
```

### Use this instead

Reject unknown variants at strict boundaries, or use a representation that deliberately preserves
the unknown value for observation or forwarding.

```rust,ignore
#[derive(serde::Deserialize)]
pub enum Permission { Read, Write }
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
