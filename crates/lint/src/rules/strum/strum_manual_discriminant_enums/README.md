# strum_manual_discriminant_enums

## What it does

Finds private unit enums that mirror every variant of a payload enum through a one-to-one `From`
conversion.

## Why is this bad?

The mirror and conversion must be updated together whenever the payload enum changes.
`EnumDiscriminants` generates that lockstep classification directly.

## Example

```rust,ignore
enum Event { Created(Item), Deleted(Id) }
enum EventKind { Created, Deleted }
impl From<&Event> for EventKind { /* exhaustive variant-only match */ }
```

## Use instead

Derive `strum::EnumDiscriminants` with the intended generated name. Keep hand-written mirror enums when
they are public or independently serialized schemas.

```rust,ignore
#[derive(strum::EnumDiscriminants)]
#[strum_discriminants(name(EventKind))]
enum Event { Created(Item), Deleted(Id) }
```
