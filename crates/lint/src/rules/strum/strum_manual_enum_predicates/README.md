# `rlib::strum_manual_enum_predicates`

## Summary

Finds complete inherent `is_*` method families that identify one enum variant apiece exactly as Strum's `EnumIs` derive does.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::strum` |
| Cargo feature | `strum` |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds complete inherent `is_*` method families that identify one enum variant apiece exactly as
Strum's `EnumIs` derive does.

## Why this matters

Each predicate repeats variant identity and must be maintained when the enum changes. `EnumIs`
generates the complete enabled-variant family from the declaration.

## Examples

### Triggers the lint

```rust,ignore
impl Message {
    fn is_write(&self) -> bool { matches!(self, Self::Write(_)) }
    fn is_quit(&self) -> bool { matches!(self, Self::Quit) }
}
```

### Use this instead

```rust,ignore
#[derive(strum::EnumIs)]
enum Message { Write(Vec<u8>), Quit }
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when the reported API is deliberate and callers depend on it.

## Settings

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `enum-variant-predicate-provider` | string | not set | Chooses the derive that should generate enum variant checks. Values: `strum_enum_is`, `derive_more_is_variant`. |

## Known limitations

No known implementation limitations.

## Related lints

None.
