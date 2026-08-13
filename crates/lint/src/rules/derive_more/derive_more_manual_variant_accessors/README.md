# derive_more_manual_variant_accessors

## What it does

Finds complete hand-written `is_<variant>` predicate families that are exactly reproducible by
`derive_more::IsVariant` when derive_more is the configured provider.

## Why is this bad?

Maintaining exhaustive variant-identity boilerplate by hand creates a second list that must change
whenever the enum changes.

## Example

```rust,ignore
impl Message {
    fn is_write(&self) -> bool { matches!(self, Self::Write(_)) }
    fn is_quit(&self) -> bool { matches!(self, Self::Quit) }
}
```

## Use instead

```rust,ignore
#[derive(derive_more::IsVariant)]
enum Message { Write(Vec<u8>), Quit }
```

When Strum is also enabled, set `derive_resolution.enum_variant_predicates` explicitly; dependency
presence never chooses between `strum::EnumIs` and `derive_more::IsVariant`.
