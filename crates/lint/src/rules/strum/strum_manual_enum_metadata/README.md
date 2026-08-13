# strum_manual_enum_metadata

## What it does

Finds exhaustive variant-to-static-metadata methods reproducible by `EnumMessage` or
`EnumProperty`.

## Why is this bad?

Parallel metadata matches scatter each variant's declaration and annotations across the impl.

## Example

```rust,ignore
impl Severity {
    fn label(&self) -> &'static str {
        match self { Self::Info => "Information", Self::Error => "Error" }
    }
}
```

## Use instead

Use `EnumMessage` for message/detailed-message vocabulary and `EnumProperty` for named static
properties. Keep policy-bearing or localized values in hand-written methods.

```rust,ignore
#[derive(strum::EnumMessage)]
enum Severity { #[strum(message = "operation failed")] Error }
```
