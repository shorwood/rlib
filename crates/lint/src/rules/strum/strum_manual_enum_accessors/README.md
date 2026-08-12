# strum_manual_enum_accessors

## What it does

Finds complete owned, shared, and mutable `Option` accessor families for every tuple variant when
they exactly match Strum's `EnumTryAs` derive.

## Why is this bad?

The authored methods repeat tuple field order, receiver behavior, and failure handling. `EnumTryAs`
keeps those accessors synchronized with the enum declaration.

## Example

```rust,ignore
impl Message {
    fn try_as_write(self) -> Option<Vec<u8>> {
        match self { Self::Write(value) => Some(value), _ => None }
    }

    fn try_as_write_ref(&self) -> Option<&Vec<u8>> {
        match self { Self::Write(value) => Some(value), _ => None }
    }

    fn try_as_write_mut(&mut self) -> Option<&mut Vec<u8>> {
        match self { Self::Write(value) => Some(value), _ => None }
    }
}
```

## Use instead

```rust,ignore
#[derive(strum::EnumTryAs)]
enum Message { Write(Vec<u8>), Quit }
```
