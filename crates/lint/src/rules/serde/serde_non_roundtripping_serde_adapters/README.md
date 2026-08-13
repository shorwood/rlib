# serde_non_roundtripping_serde_adapters

## What it does

Finds paired `serialize_with` and `deserialize_with` helpers whose names identify the same adapter
family but different units or encodings.

## Why is this bad?

The serialized representation is decoded under a different scale or codec, so a nominal round trip
changes the value or fails despite both directions compiling independently.

## Example

```rust,ignore
#[serde(
    serialize_with = "timestamp_seconds",
    deserialize_with = "timestamp_milliseconds"
)]
created_at: Timestamp,
```

## Use instead

Pair helpers from the same unit or encoding contract.

```rust,ignore
#[serde(
    serialize_with = "timestamp_seconds",
    deserialize_with = "timestamp_seconds"
)]
created_at: Timestamp,
```
