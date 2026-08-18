# `rlib::bon_parameter_heavy_apis_without_builders`

## Summary

Finds exported free functions and inherent methods with enough positional parameters and ambiguity that a Bon named builder would materially improve calls.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::bon` |
| Cargo feature | `bon` |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds exported free functions and inherent methods with enough positional parameters and ambiguity
that a Bon named builder would materially improve calls. Trait methods are excluded because Bon's
function-builder attribute does not support trait methods.

## Why this matters

Long positional calls make boolean policy and same-representation domain values easy to transpose.
Callers must repeatedly reconstruct parameter meaning from the declaration.

## Examples

### Triggers the lint

```rust,ignore
pub fn render(path: PathBuf, width: u32, height: u32, cache: bool, color: bool) {}
```

### Use this instead

Use Bon when the values are independent call-time choices; prefer a named domain options type when
they form a reusable concept:

```rust,ignore
#[bon::builder]
pub fn render(path: PathBuf, width: u32, height: u32, cache: bool, color: bool) {}
```

## What it skips

Trait methods are excluded because Bon's function-builder attribute does not support them.

## When to turn it off

Turn this lint off when the reported API is deliberate and callers depend on it.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
