# `rlib::non_adjacent_extension_trait_impls`

## Summary

Checks that a local extension trait and every hand-written foreign-target or blanket impl form one consecutive declaration group in the same module.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Checks that a local extension trait and every hand-written foreign-target or blanket impl form
one consecutive declaration group in the same module. Comments and attributes may appear
between them because they do not constitute independent module declarations.

## Why this matters

Separating a trait from its implementation makes a small extension look
like two unrelated APIs and forces readers to search for the behavior. Cross-module impls
also make it easy to add an impl without discovering the trait's existing scope and naming
rationale.

## Examples

### Triggers the lint

```rust
trait ItemExt {
    fn inspect(&self);
}

struct AnalysisState;

impl ItemExt for Item<'_> {
    fn inspect(&self) {}
}
```

### Use this instead

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

## What it skips

Comments and attributes may appear between them because they do not constitute independent module declarations.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

- [`rlib::non_adjacent_struct_impls`](../non_adjacent_struct_impls/README.md) — Applies the same grouping rule to struct implementations.
