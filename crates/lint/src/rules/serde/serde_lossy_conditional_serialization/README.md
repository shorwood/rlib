# `rlib::serde_lossy_conditional_serialization`

## Summary

Finds conditionally serialized required fields that have neither optionality nor a deserialization default.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::serde` |
| Cargo feature | `serde` |
| Purpose | Safety |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds conditionally serialized required fields that have neither optionality nor a deserialization
default.

## Why this matters

The serializer can omit the field, but the same type's deserializer rejects the resulting missing
key. A value can therefore fail to round-trip solely because it satisfies the skip predicate.

## Examples

### Triggers the lint

```rust,ignore
#[derive(serde::Serialize, serde::Deserialize)]
struct Profile {
    #[serde(skip_serializing_if = "String::is_empty")]
    display_name: String,
}
```

### Use this instead

Make omission reversible with optionality or a matching default policy.

```rust,ignore
struct Profile {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    display_name: String,
}
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off only when the risk is handled elsewhere and documented.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
