# `rlib::strum_manual_discriminant_enums`

## Summary

Finds private unit enums that mirror every variant of a payload enum through a one-to-one `From` conversion.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::strum` |
| Cargo feature | `strum` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds private unit enums that mirror every variant of a payload enum through a one-to-one `From`
conversion.

## Why this matters

The mirror and conversion must be updated together whenever the payload enum changes.
`EnumDiscriminants` generates that lockstep classification directly.

## Examples

### Triggers the lint

```rust,ignore
enum Event { Created(Item), Deleted(Id) }
enum EventKind { Created, Deleted }
impl From<&Event> for EventKind { /* exhaustive variant-only match */ }
```

### Use this instead

Derive `strum::EnumDiscriminants` with the intended generated name. Keep hand-written mirror enums when
they are public or independently serialized schemas.

```rust,ignore
#[derive(strum::EnumDiscriminants)]
#[strum_discriminants(name(EventKind))]
enum Event { Created(Item), Deleted(Id) }
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
