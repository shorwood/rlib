# strum_manual_enum_counts

## What it does

Finds inherent constants and zero-argument methods whose integer value exactly repeats an enum's
active variant count when Strum's `EnumCount` derive is directly available.

## Why is this bad?

A literal total is disconnected from the enum declaration and can become stale after variants are
added or removed.

## Example

```rust,ignore
impl Stage {
    const COUNT: usize = 2;
}
```

## Use instead

```rust,ignore
#[derive(strum::EnumCount)]
enum Stage { Planned, Complete }
```
