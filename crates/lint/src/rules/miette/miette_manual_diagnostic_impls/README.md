# `rlib::miette_manual_diagnostic_impls`

## Summary

Finds hand-written `miette::Diagnostic` implementations containing only defaults, static metadata, or direct source-field forwarding expressible by Miette derive attributes.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::miette` |
| Cargo feature | `miette` |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds hand-written `miette::Diagnostic` implementations containing only defaults, static metadata, or
direct source-field forwarding expressible by Miette derive attributes.

## Why this matters

Hand-written diagnostic code makes it harder to see which implementations truly need custom
labels, related errors, translated text, or values calculated at runtime.

## Examples

### Triggers the lint

```rust,ignore
impl miette::Diagnostic for Marker {}
```

### Use this instead

```rust,ignore
#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("marker")]
struct Marker;
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
