# `rlib::derive_more_opaque_derived_display_contracts`

## Summary

Finds values with derived `Display` whose `to_string()` output is used as a standard map key.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::derive_more` |
| Cargo feature | `derive_more` |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds values with derived `Display` whose `to_string()` output is used as a standard map key.

## Why this matters

Using presentation output as machine identity silently turns formatting changes into cache misses,
key collisions, or persistence incompatibilities.

## Examples

### Triggers the lint

```rust,ignore
#[derive(derive_more::Display)]
#[display("{kind}:{value}")]
struct CacheKey { kind: Kind, value: String }

cache.insert(key.to_string(), record);
```

### Use this instead

Define and use a named encoding with an explicit compatibility promise.

```rust,ignore
impl CacheKey {
    fn encode(&self) -> String { /* stable key grammar */ }
}

cache.insert(key.encode(), record);
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
