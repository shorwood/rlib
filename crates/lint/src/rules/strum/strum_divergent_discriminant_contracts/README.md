# `rlib::strum_divergent_discriminant_contracts`

## Summary

Finds generated `EnumDiscriminants` types that also derive serialization and therefore become an independently observed schema.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::strum` |
| Cargo feature | `strum` |
| Purpose | Correctness |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds generated `EnumDiscriminants` types that also derive serialization and therefore become an
independently observed schema.

## Why this matters

The generated type evolves in lockstep with the payload enum. Persistence or protocol compatibility
often needs an independently versioned declaration instead.

## Examples

### Triggers the lint

```rust,ignore
#[derive(strum::EnumDiscriminants)]
#[strum_discriminants(derive(serde::Serialize, serde::Deserialize))]
pub enum Event { Created(Item), Deleted(Id) }
```

### Use this instead

Use a hand-written schema enum with explicit conversions when independent compatibility matters.

```rust,ignore
#[derive(Serialize)]
enum EventSchema { Created, Deleted }
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off only when the reported behavior is intentional and covered by tests.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
