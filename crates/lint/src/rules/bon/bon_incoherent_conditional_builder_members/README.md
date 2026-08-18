# `rlib::bon_incoherent_conditional_builder_members`

## Summary

Finds Bon members on derived structs, free functions, and associated functions whose requiredness, default, hidden initialization, or positional placement is changed with `cfg_attr` while the member itself remains present.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::bon` |
| Cargo feature | `bon` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds Bon members on derived structs, free functions, and associated functions whose
requiredness, default, hidden initialization, or positional placement is changed with
`cfg_attr` while the member itself remains present. Words in the `cfg` condition do not matter
unless the `builder(...)` attributes actually change.

## Why this matters

The same builder then accepts different calls under different features or targets. Callers cannot
rely on one stable set of required fields or one way to finish the build.

## Examples

### Triggers the lint

```rust,ignore
#[bon::builder]
fn connect(#[cfg_attr(feature = "lenient", builder(default))] timeout: u64) {}
```

### Use this instead

Keep the construction policy stable, or explicitly gate a distinct capability/API.

```rust,ignore
#[bon::builder]
fn connect(#[builder(default)] timeout: u64) {}
```

## What it skips

It ignores `cfg` conditions that do not change the member's `builder(...)` attributes.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
