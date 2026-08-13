# non_adjacent_struct_impls

## What it does

Checks that all impl blocks for a struct form one group immediately after the struct.

## Why is this bad?

Keeping a struct and its behavior together makes the type easier to understand without
searching through the rest of the module.

For example, this helper splits the struct from its implementation:

## Example

```rust
struct Cache;
fn cache_directory() -> &'static str { "/tmp" }
impl Cache {
    fn clear(&mut self) {}
}
```

## Use instead

Keep the complete impl group directly after the definition:

```rust
struct Cache;
impl Cache {
    fn clear(&mut self) {}
}
fn cache_directory() -> &'static str { "/tmp" }
```
