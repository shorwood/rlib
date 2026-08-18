# `rlib::strum_manual_enum_accessors`

## Summary

Finds complete owned, shared, and mutable `Option` accessor families for every tuple variant when they exactly match Strum's `EnumTryAs` derive.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::strum` |
| Cargo feature | `strum` |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds complete owned, shared, and mutable `Option` accessor families for every tuple variant when
they exactly match Strum's `EnumTryAs` derive.

## Why this matters

The hand-written methods repeat tuple field order, receiver behavior, and failure handling. `EnumTryAs`
keeps those accessors synchronized with the enum declaration.

## Examples

### Triggers the lint

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

### Use this instead

```rust,ignore
#[derive(strum::EnumTryAs)]
enum Message { Write(Vec<u8>), Quit }
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
