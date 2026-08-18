# `rlib::serde_sensitive_fields_serialized_by_default`

## Summary

Finds public derived Serde serializers that include strongly named credential fields stored in raw string or byte carrier types without an explicit serialization policy.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::serde` |
| Cargo feature | `serde` |
| Purpose | Safety |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds public derived Serde serializers that include strongly named credential fields stored in raw
string or byte carrier types without an explicit serialization policy.

## Why this matters

Derived serialization is frequently reused by logs, responses, caches, and diagnostics. Including
raw secrets by default can disclose credentials through a boundary far from the type declaration.

## Examples

### Triggers the lint

```rust,ignore
#[derive(serde::Serialize)]
pub struct Session { pub access_token: String }
```

### Use this instead

Skip the field, use a redacting/encrypting serializer, or store it in a secret-aware wrapper with an
intentional serialization choice.

```rust,ignore
#[derive(serde::Serialize)]
pub struct Session {
    #[serde(skip_serializing)]
    pub access_token: String,
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
