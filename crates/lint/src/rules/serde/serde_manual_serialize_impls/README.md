# `rlib::serde_manual_serialize_impls`

## Summary

Finds `Serialize` implementations on single-field newtypes that forward the sole field unchanged to the provided serializer.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::serde` |
| Cargo feature | `serde` |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds `Serialize` implementations on single-field newtypes that forward the sole field unchanged to
the provided serializer.

## Why this matters

Transparent forwarding boilerplate obscures the manual serializers that actually define a custom
wire format, validate values, or branch by format capability.

## Examples

### Triggers the lint

```rust,ignore
impl serde::Serialize for UserId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: serde::Serializer {
        self.0.serialize(serializer)
    }
}
```

### Use this instead

```rust,ignore
#[derive(serde::Serialize)]
#[serde(transparent)]
struct UserId(u64);
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when the reported API is deliberate and callers depend on it.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
