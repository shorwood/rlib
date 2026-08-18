# `rlib::bon_needless_builders_for_small_apis`

## Summary

Finds uncustomized private Bon free-function and associated-function builders with only one or two required parameters.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::bon` |
| Cargo feature | `bon` |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds uncustomized private Bon free-function and associated-function builders with only one or two
required parameters. Method receivers are not counted as builder members.

## Why this matters

For a small, stable, internal signature, generated builder state and setter APIs add navigation and
compile-time cost without providing optionality, defaults, conversion, or staging value.

## Examples

### Triggers the lint

```rust,ignore
#[bon::builder]
fn checksum(path: PathBuf, retries: u32) -> Digest { /* ... */ }
```

### Use this instead

Keep the direct call when both arguments are distinct and self-explanatory:

```rust,ignore
fn checksum(path: PathBuf, retries: u32) -> Digest { /* ... */ }
```

## What it skips

Method receivers are not counted as builder members.

## When to turn it off

Turn this lint off when the reported API is deliberate and callers depend on it.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
