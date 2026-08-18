# `rlib::derive_more_manual_forwarding_interfaces`

## Summary

Finds `AsRef`, `AsMut`, `Deref`, `DerefMut`, `Index`, and `IndexMut` implementations that expose or forward to one stored field without adding policy.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::derive_more` |
| Cargo feature | `derive_more` |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds `AsRef`, `AsMut`, `Deref`, `DerefMut`, `Index`, and `IndexMut` implementations that expose or
forward to one stored field without adding policy.

## Why this matters

Hand-written forwarding implementations obscure whether an interface is a direct projection and make
the immutable and mutable forms can drift apart.

## Examples

### Triggers the lint

```rust,ignore
impl Deref for SystemList {
    type Target = [System];
    fn deref(&self) -> &Self::Target { &self.0 }
}
```

### Use this instead

Declare the forwarding family on the wrapper and select `forward` when the interface should pass
through the field's own implementation.

```rust,ignore
#[derive(derive_more::Deref)]
#[deref(forward)]
struct SystemList(Vec<System>);
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when the reported API is deliberate and callers depend on it.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
