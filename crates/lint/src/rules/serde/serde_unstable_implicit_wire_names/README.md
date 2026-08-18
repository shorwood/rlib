# `rlib::serde_unstable_implicit_wire_names`

## Summary

Finds public Serde types whose derived field or variant names are used in serialized data without being stated explicitly.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::serde` |
| Cargo feature | `serde` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds public Serde types whose derived field or variant names are used in serialized data without
being stated explicitly.

## Why this matters

An ordinary Rust rename then silently changes externally observed data. Explicit Serde naming makes
that compatibility decision visible and keeps refactors from rewriting a protocol or persisted
schema.

## Examples

### Triggers the lint

```rust,ignore
#[derive(serde::Serialize, serde::Deserialize)]
pub enum ApiEvent { UserCreated, UserDeleted }
```

### Use this instead

Declare the wire names independently of Rust identifiers.

```rust,ignore
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiEvent { UserCreated, UserDeleted }
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

- [`rlib::serde_duplicate_serialized_names`](../serde_duplicate_serialized_names/README.md) — Finds explicit or derived wire names that collide.
