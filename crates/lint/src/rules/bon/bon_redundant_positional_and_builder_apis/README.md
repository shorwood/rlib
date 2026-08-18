# `rlib::bon_redundant_positional_and_builder_apis`

## Summary

Finds complex externally reachable Bon function builders that keep most inputs positional through exact member-level `start_fn` or `finish_fn` configuration.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::bon` |
| Cargo feature | `bon` |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds complex externally reachable Bon function builders that keep most inputs positional through
exact member-level `start_fn` or `finish_fn` configuration.

## Why this matters

The builder adds generated API without providing a substantially named call API. Long
positional start and finish calls retain the ordering mistakes that the builder should prevent.

## Examples

### Triggers the lint

```rust,ignore
#[bon::builder]
pub fn render(
    #[builder(start_fn)] path: PathBuf,
    #[builder(start_fn)] width: u32,
    #[builder(start_fn)] height: u32,
    #[builder(finish_fn)] cache: bool,
    color: bool,
) {}
```

### Use this instead

Keep only a small identity-bearing prefix positional and make the rest named setters.

```rust,ignore
#[bon::builder]
pub fn render(#[builder(start_fn)] path: PathBuf, width: u32, height: u32, cache: bool, color: bool) {}
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
