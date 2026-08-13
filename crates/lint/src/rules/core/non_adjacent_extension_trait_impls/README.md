# non_adjacent_extension_trait_impls

## What it does

Checks that a local extension trait and every hand-written foreign-target or blanket impl form
one consecutive declaration group in the same module. Comments and attributes may appear
between them because they do not constitute independent module declarations.

## Why is this bad?

Separating the contract from its implementation makes a small extension abstraction look
like two unrelated APIs and forces readers to search for the behavior. Cross-module impls
also make it easy to add an impl without discovering the trait's existing scope and naming
rationale.

## Example

```rust
trait ItemExt {
    fn inspect(&self);
}

struct AnalysisState;

impl ItemExt for Item<'_> {
    fn inspect(&self) {}
}
```

## Use instead

Keep the complete extension beside its declaration:

```rust
trait ItemExt {
    fn inspect(&self);
}

impl ItemExt for Item<'_> {
    fn inspect(&self) {}
}

struct AnalysisState;
```
