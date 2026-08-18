# `rlib::incoherent_extension_traits`

## Summary

Checks local traits implemented for a foreign named type or a generic blanket target.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Checks local traits implemented for a foreign named type or a generic blanket target.
Such an extension trait must operate on at most one concrete nonreceiver subject family and
contain no more than `extension_traits.max_methods` methods, which defaults to eight.
Receiver-only accessors count toward the method budget but do not invent a subject family.
Primitive, borrowed or owned string, and generic callback parameters likewise do not split a
family.

## Why this matters

An extension trait is useful when it gives one foreign type a focused names. A trait
that accumulates unrelated subjects becomes a disguised utility module, while a large trait
makes every import expose a collection of unrelated methods and encourages agents to append the next
convenient helper to the same catch-all abstraction.

## Examples

### Triggers the lint

```rust
trait ContextExt {
    fn inspect_item(&self, item: &Item<'_>);
    fn inspect_expression(&self, expression: &Expr<'_>);
}
```

### Use this instead

Prefer traits named and scoped around one subject:

```rust
trait ItemContextExt {
    fn inspect_item(&self, item: &Item<'_>);
}

trait ExpressionContextExt {
    fn inspect_expression(&self, expression: &Expr<'_>);
}
```

## What it skips

Receiver-only accessors count toward the method budget but do not invent a subject family. Primitive, borrowed or owned string, and generic callback parameters likewise do not split a family.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `extension-trait-methods-threshold` | positive integer | `8` | Sets how many methods make an extension trait broad enough to require one clear subject. |

## Known limitations

No known implementation limitations.

## Related lints

None.
