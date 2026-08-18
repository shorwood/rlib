# `rlib::cross_file_struct_impls`

## Summary

Checks that every direct impl block for a local struct is defined in the same physical file as the struct.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Checks that every direct impl block for a local struct is defined in the same physical file
as the struct.

Diagnostics show the shortest path suffixes that distinguish the two files, so identically
named files in different modules remain unambiguous.

## Why this matters

Keeping a struct and all of its behavior in one file makes the type understandable without
searching through unrelated modules. The definition-order lint separately ensures that impl
blocks in the struct's module immediately follow its definition.

For example, this layout separates a type from its behavior:

## Examples

### Triggers the lint

```rust,ignore
// model.rs
pub struct User;

// service.rs
impl User {
    pub fn name(&self) -> &str { "Ada" }
}
```

### Use this instead

Keeping both declarations in the owning file makes the type self-contained:

```rust
pub struct User;

impl User {
    pub fn name(&self) -> &str { "Ada" }
}
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

- [`rlib::non_adjacent_struct_impls`](../non_adjacent_struct_impls/README.md) — Keeps same-file implementations beside their struct.
