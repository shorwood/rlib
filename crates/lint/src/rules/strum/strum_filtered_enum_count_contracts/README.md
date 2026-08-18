# `rlib::strum_filtered_enum_count_contracts`

## Summary

Finds `EnumCount::COUNT` used by APIs claiming an enabled, visible, supported, actionable, or available subset when the enum contains variants excluded from that domain.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::strum` |
| Cargo feature | `strum` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds `EnumCount::COUNT` used by APIs claiming an enabled, visible, supported, actionable, or
available subset when the enum contains variants excluded from that domain.

## Why this matters

`COUNT` includes deprecated, sentinel, and payload variants even when a surrounding name promises a
narrower set. Strum itself omits variants marked `disabled` from the generated count.

## Examples

### Triggers the lint

```rust,ignore
fn enabled_stage_count() -> usize { Stage::COUNT }
```

### Use this instead

Count the explicitly declared subset or rename the API to communicate that it returns a total.

```rust,ignore
fn supported_count() -> usize { Capability::iter().filter(Capability::is_supported).count() }
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
