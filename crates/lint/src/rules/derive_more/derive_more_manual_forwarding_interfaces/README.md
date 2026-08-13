# derive_more_manual_forwarding_interfaces

## What it does

Finds `AsRef`, `AsMut`, `Deref`, `DerefMut`, `Index`, and `IndexMut` implementations that expose or
forward to one stored field without adding policy.

## Why is this bad?

Authored forwarding implementations obscure whether an interface is a direct projection and make
immutable and mutable companion contracts drift independently.

## Example

```rust,ignore
impl Deref for SystemList {
    type Target = [System];
    fn deref(&self) -> &Self::Target { &self.0 }
}
```

## Use instead

Declare the forwarding family on the wrapper and select `forward` when the interface should pass
through the field's own implementation.

```rust,ignore
#[derive(derive_more::Deref)]
#[deref(forward)]
struct SystemList(Vec<System>);
```
