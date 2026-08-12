# strum_divergent_discriminant_contracts

## What it does

Finds generated `EnumDiscriminants` types that also derive serialization and therefore become an
independently observed schema.

## Why is this bad?

The generated type evolves in lockstep with the payload enum. Persistence or protocol compatibility
often needs an independently versioned declaration instead.

## Example

```rust,ignore
#[derive(strum::EnumDiscriminants)]
#[strum_discriminants(derive(serde::Serialize, serde::Deserialize))]
pub enum Event { Created(Item), Deleted(Id) }
```

## Use instead

Use an authored schema enum with explicit conversions when independent compatibility matters.

```rust,ignore
#[derive(Serialize)]
enum EventSchema { Created, Deleted }
```
