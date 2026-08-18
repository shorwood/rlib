# `rlib::derive_more_manual_variant_accessors`

## Summary

Finds complete hand-written `is_<variant>` predicate families that are exactly reproducible by `derive_more::IsVariant` when derive_more is the configured provider.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::derive_more` |
| Cargo feature | `derive_more` |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds complete hand-written `is_<variant>` predicate families that are exactly reproducible by
`derive_more::IsVariant` when derive_more is the configured provider.

## Why this matters

Maintaining exhaustive variant-identity boilerplate by hand creates a second list that must change
whenever the enum changes.

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
#[derive(derive_more::IsVariant)]
enum Message { Write(Vec<u8>), Quit }
```

When Strum is also enabled, set `derive_resolution.enum_variant_predicates` explicitly; dependency
presence never chooses between `strum::EnumIs` and `derive_more::IsVariant`.

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when the reported API is deliberate and callers depend on it.

## Settings

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `enum-variant-predicate-provider` | string | not set | Chooses which derive should generate enum variant checks. Values: `derive_more_is_variant`, `strum_enum_is`. |

## Known limitations

No known implementation limitations.

## Related lints

None.
