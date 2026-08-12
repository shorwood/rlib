# strum_manual_enum_string_conversions

## What it does

Finds exhaustive `&'static str` enum methods equivalent to Strum's `AsRefStr` naming contract.

## Why is this bad?

The match duplicates every variant spelling and can drift from parsing and advertised names.

## Example

```rust,ignore
impl Stage {
    fn as_str(&self) -> &'static str {
        match self { Self::Planned => "Planned", Self::Active => "Active" }
    }
}
```

## Use instead

Derive `strum::AsRefStr`; review named-method callers before removing a public compatibility shim.

```rust,ignore
#[derive(strum::AsRefStr)]
enum Stage { Planned, Complete }
```
