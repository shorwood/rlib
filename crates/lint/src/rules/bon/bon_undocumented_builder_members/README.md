# `rlib::bon_undocumented_builder_members`

## Summary

Finds non-obvious optionality, default, conversion, or hidden-initialization behavior on undocumented members of externally reachable Bon struct, free-function, and associated-function builders.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::bon` |
| Cargo feature | `bon` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds non-obvious optionality, default, conversion, or hidden-initialization behavior on undocumented
members of externally reachable Bon struct, free-function, and associated-function builders.
Documentation must contain substantive hand-written text; empty `doc` attributes and unrelated `doc`
identifiers in converter expressions do not suppress the lint.

## Why this matters

Generated signatures show how to call a setter, but they cannot explain what omission, conversion,
validation, or hidden initialization means in the domain rule.

## Examples

### Triggers the lint

```rust,ignore
#[bon::builder]
pub fn connect(#[builder(default = 30)] timeout_seconds: u64) {}
```

### Use this instead

Document the behavior on the hand-written field or parameter so Bon can carry it to generated APIs:

```rust,ignore
#[bon::builder]
pub fn connect(
    /// Connection timeout in seconds; defaults to 30.
    #[builder(default = 30)]
    timeout_seconds: u64,
) {}
```

## What it skips

Documentation must contain substantive hand-written text; empty `doc` attributes and unrelated `doc` identifiers in converter expressions do not suppress the lint.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
