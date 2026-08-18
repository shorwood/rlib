# `rlib::bon_public_builder_implementation_types`

## Summary

Finds generated Bon builder or generated builder state definitions named in hand-written exported function, inherent-method, or field types, including through nested containers and pointer types.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::bon` |
| Cargo feature | `bon` |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds generated Bon builder or generated builder state definitions named in hand-written exported
function, inherent-method, or field types, including through nested containers and pointer types.

## Why this matters

Generated types encode macro representation and required-member state. Publishing them makes a
macro upgrade or builder-policy change part of the library's compatibility promise.

## Examples

### Triggers the lint

```rust,ignore
#[derive(bon::Builder)]
pub struct Request { host: String }

pub fn partially_configured() -> RequestBuilder { Request::builder() }
```

### Use this instead

Expose completed domain values or define an intentional domain abstraction for staged construction.

```rust,ignore
pub fn configured_request() -> Request {
    Request::builder().host("example.test".to_owned()).build()
}
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
