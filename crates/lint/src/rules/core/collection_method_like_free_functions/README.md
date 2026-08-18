# `rlib::collection_method_like_free_functions`

## Summary

Checks for free functions whose first parameter is a vector or slice of a struct defined in the same crate.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Checks for free functions whose first parameter is a vector or slice of a struct defined in
the same crate.

An existing standard `<Element>List` wrapper is reused only when its `items` field stores
the same resolved element instantiation. If that name belongs to an incompatible type, the
diagnostic asks for a separately named wrapper instead.

## Why this matters

A collection of domain values usually has behavior of its own. Giving that collection a
name keeps its behavior discoverable and prevents unrelated free functions from becoming
the collection's informal interface.

For example, this function leaves the collection without a home for its behavior:

## Examples

### Triggers the lint

```rust
struct Item;

fn inspect(items: &[Item]) {}
```

### Use this instead

A small wrapper makes the intended interface explicit:

```rust
struct ItemList {
    items: Vec<Item>,
}

impl ItemList {
    fn inspect(&self) {}
}
```

## What it skips

An existing standard `<Element>List` wrapper is reused only when its `items` field stores the same resolved element instantiation.

## When to turn it off

Turn this lint off when the reported API is deliberate and callers depend on it.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
