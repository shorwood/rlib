# `rlib::serde_asymmetric_serde_contracts`

## Summary

Finds undocumented directional Serde renames whose serialization and deserialization names differ on a type deriving both traits.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::serde` |
| Cargo feature | `serde` |
| Purpose | Correctness |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds undocumented directional Serde renames whose serialization and deserialization names differ
on a type deriving both traits.

## Why this matters

The type writes one schema and reads another. Without an explicit compatibility explanation,
round-trip expectations and future migrations are unclear.

## Examples

### Triggers the lint

```rust,ignore
#[derive(serde::Serialize, serde::Deserialize)]
struct Account {
    #[serde(rename(serialize = "user_id", deserialize = "id"))]
    id: u64,
}
```

### Use this instead

Use one stable name, or document the intentional migration bridge at the renamed declaration.

```rust,ignore
struct Account {
    #[serde(rename = "user_id")]
    id: u64,
}
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
