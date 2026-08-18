# `rlib::misordered_inherent_impl_items`

## Summary

Orders each inherent impl dependency-first, then by associated item kind and visibility.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Style |
| Default level | `warn` |
| Fix | Automatic |

## What it catches

Orders each inherent impl dependency-first, then by associated item kind and visibility.
Trait impls and other inherent impl blocks are independent.

## Why this matters

A stable order makes an impl predictable to scan: supporting types and constants come
first, followed by constructors, other associated functions, and methods. Dependency-first
ordering also prevents a declaration from relying on details introduced later in the block.
Constructor ranking follows resolved return types, so `Self`, the concrete type name, and aliases
receive the same ordering semantics.

For example, this method appears before the constructor it uses:

## Examples

### Triggers the lint

```rust
struct Session;

impl Session {
    fn reset(&mut self) { *self = Self::new(); }
    fn new() -> Self { Self }
}
```

### Use this instead

Put the constructor before the dependent method:

```rust
struct Session;

impl Session {
    fn new() -> Self { Self }
    fn reset(&mut self) { *self = Self::new(); }
}
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when your project deliberately follows a different style.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
