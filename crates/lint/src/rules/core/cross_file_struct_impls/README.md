# cross_file_struct_impls

## What it does

Checks that every direct impl block for a local struct is defined in the same physical file
as the struct.

Diagnostics show the shortest path suffixes that distinguish the two files, so identically
named files in different modules remain unambiguous.

## Why is this bad?

Keeping a struct and all of its behavior in one file makes the type understandable without
searching through unrelated modules. The definition-order lint separately ensures that impl
blocks in the struct's module immediately follow its definition.

For example, this layout separates a type from its behavior:

## Example

```rust,ignore
// model.rs
pub struct User;

// service.rs
impl User {
    pub fn name(&self) -> &str { "Ada" }
}
```

## Use instead

Keeping both declarations in the owning file makes the type self-contained:

```rust
pub struct User;

impl User {
    pub fn name(&self) -> &str { "Ada" }
}
```
