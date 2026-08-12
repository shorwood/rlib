# strum_filtered_enum_count_contracts

## What it does

Finds `EnumCount::COUNT` used by APIs claiming an enabled, visible, supported, actionable, or
available subset when the enum contains variants excluded from that domain.

## Why is this bad?

`COUNT` is the total declaration count. It includes disabled, deprecated, sentinel, and payload
variants even when a surrounding name promises a narrower set.

## Example

```rust,ignore
fn enabled_stage_count() -> usize { Stage::COUNT }
```

## Use instead

Count the explicitly declared subset or rename the API to communicate that it returns a total.

```rust,ignore
fn supported_count() -> usize { Capability::iter().filter(Capability::is_supported).count() }
```
