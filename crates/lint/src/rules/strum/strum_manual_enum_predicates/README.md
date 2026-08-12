# strum_manual_enum_predicates

## What it does

Finds complete inherent `is_*` method families that identify one enum variant apiece exactly as
Strum's `EnumIs` derive does.

## Why is this bad?

Each predicate repeats variant identity and must be maintained when the enum changes. `EnumIs`
generates the complete enabled-variant family from the declaration.

## Example

```rust,ignore
impl Message {
    fn is_write(&self) -> bool { matches!(self, Self::Write(_)) }
    fn is_quit(&self) -> bool { matches!(self, Self::Quit) }
}
```

## Use instead

```rust,ignore
#[derive(strum::EnumIs)]
enum Message { Write(Vec<u8>), Quit }
```
