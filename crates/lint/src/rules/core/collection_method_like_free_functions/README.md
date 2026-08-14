# collection_method_like_free_functions

## What it does

Checks for free functions whose first parameter is a vector or slice of a struct defined in
the same crate.

An existing canonical `<Element>List` wrapper is reused only when its `items` field stores
the same resolved element instantiation. If that name belongs to an incompatible type, the
diagnostic asks for a separately named wrapper instead.

## Why is this bad?

A collection of domain values usually has behavior of its own. Giving that collection a
name keeps its behavior discoverable and prevents unrelated free functions from becoming
the collection's informal interface.

For example, this function leaves the collection without a home for its behavior:

## Example

```rust
struct Item;

fn inspect(items: &[Item]) {}
```

## Use instead

A small wrapper makes the intended interface explicit:

```rust
struct ItemList {
    items: Vec<Item>,
}

impl ItemList {
    fn inspect(&self) {}
}
```
