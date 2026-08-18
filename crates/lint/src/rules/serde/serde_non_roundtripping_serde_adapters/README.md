# `rlib::serde_non_roundtripping_serde_adapters`

## Summary

Finds paired `serialize_with` and `deserialize_with` helpers whose names identify the same adapter family but different units or encodings.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::serde` |
| Cargo feature | `serde` |
| Purpose | Correctness |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds paired `serialize_with` and `deserialize_with` helpers whose names identify the same adapter
family but different units or encodings.

## Why this matters

The serialized representation is decoded under a different scale or codec, so a named round trip
changes the value or fails despite both directions compiling independently.

## Examples

### Triggers the lint

```rust,ignore
#[serde(
    serialize_with = "timestamp_seconds",
    deserialize_with = "timestamp_milliseconds"
)]
created_at: Timestamp,
```

### Use this instead

Pair helpers that use the same unit or encoding.

```rust,ignore
#[serde(
    serialize_with = "timestamp_seconds",
    deserialize_with = "timestamp_seconds"
)]
created_at: Timestamp,
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
