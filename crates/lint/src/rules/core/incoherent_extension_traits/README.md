# incoherent_extension_traits

## What it does

Checks local traits implemented for a foreign nominal type or a generic blanket target.
Such an extension trait must operate on at most one concrete nonreceiver subject family and
contain no more than `extension_traits.max_methods` methods, which defaults to eight.
Receiver-only accessors count toward the method budget but do not invent a subject family.
Primitive, string, and generic callback parameters likewise do not split a family.

## Why is this bad?

An extension trait is useful when it gives one foreign type a focused vocabulary. A trait
that accumulates unrelated subjects becomes a disguised utility module, while a large trait
makes every import expose an incoherent surface and encourages agents to append the next
convenient helper to the same catch-all abstraction.

## Example

```rust
trait ContextExt {
    fn inspect_item(&self, item: &Item<'_>);
    fn inspect_expression(&self, expression: &Expr<'_>);
}
```

## Use instead


Prefer traits named and scoped around one subject:

```rust
trait ItemContextExt {
    fn inspect_item(&self, item: &Item<'_>);
}

trait ExpressionContextExt {
    fn inspect_expression(&self, expression: &Expr<'_>);
}
```
