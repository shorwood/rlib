# `rlib::strum_manual_enum_metadata`

## Summary

Finds exhaustive variant-to-static-metadata methods reproducible by `EnumMessage` or `EnumProperty`.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::strum` |
| Cargo feature | `strum` |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds exhaustive variant-to-static-metadata methods reproducible by `EnumMessage` or
`EnumProperty`.

## Why this matters

Parallel metadata matches scatter each variant's declaration and annotations across the impl.

## Examples

### Triggers the lint

```rust,ignore
impl Severity {
    fn label(&self) -> &'static str {
        match self { Self::Info => "Information", Self::Error => "Error" }
    }
}
```

### Use this instead

Use `EnumMessage` for message/detailed-message names and `EnumProperty` for named static
properties. Keep policy-bearing or localized values in hand-written methods.

```rust,ignore
#[derive(strum::EnumMessage)]
enum Severity { #[strum(message = "operation failed")] Error }
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
