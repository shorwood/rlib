# misordered_inherent_impl_items

## What it does

Orders each inherent impl dependency-first, then by associated item kind and visibility.
Trait impls and other inherent impl blocks are independent.

## Why is this bad?

A stable order makes an impl predictable to scan: supporting types and constants come
first, followed by constructors, other associated functions, and methods. Dependency-first
ordering also prevents a declaration from relying on details introduced later in the block.

For example, this method appears before the constructor it uses:

## Example

```rust
struct Session;

impl Session {
    fn reset(&mut self) { *self = Self::new(); }
    fn new() -> Self { Self }
}
```

## Use instead


Put the constructor before the dependent method:

```rust
struct Session;

impl Session {
    fn new() -> Self { Self }
    fn reset(&mut self) { *self = Self::new(); }
}
```
