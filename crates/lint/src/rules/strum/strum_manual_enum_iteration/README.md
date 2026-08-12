# strum_manual_enum_iteration

## What it does

Finds authored functions, methods, simple `IntoIterator` implementations, and iterator adapters
that yield every unit enum variant exactly once in declaration order when Strum is a direct
dependency.

## Why is this bad?

Manual enumeration duplicates the enum declaration and can silently become stale when a variant is
added. `EnumIter` keeps the enumeration protocol attached to its semantic owner.

## Example

```rust,ignore
fn stages() -> impl Iterator<Item = Stage> {
    [Stage::Planned, Stage::Complete].into_iter()
}
```

## Use instead

```rust,ignore
#[derive(strum::EnumIter)]
enum Stage { Planned, Complete }
```
