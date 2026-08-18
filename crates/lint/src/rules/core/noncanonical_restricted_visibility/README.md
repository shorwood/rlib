# `rlib::noncanonical_restricted_visibility`

## Summary

Restricts hand-written definitions to four clear visibility levels: private, `pub(super)`, `pub(crate)`, and unrestricted `pub`.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Style |
| Default level | `warn` |
| Fix | Automatic |

## What it catches

Restricts hand-written definitions to four clear visibility levels:
private, `pub(super)`, `pub(crate)`, and unrestricted `pub`. It rejects `pub(self)`,
`pub(in self)`, `pub(in super)`, `pub(in crate)`, and arbitrary `pub(in path)` syntax on
modules, types, functions, constants, statics, struct and union fields, and associated items.
Import reexports are governed separately by `non_defining_module_reexports`.

Exact aliases receive machine-applicable standard edits. Arbitrary ancestor restrictions
do not: choosing between relocation and crate-wide ownership requires design context.
Macro-generated visibility is ignored because its source belongs to the macro definition.

## Why this matters

`pub(in path)` ties a declaration to one particular module path. Readers must resolve that path
before they can tell who may use the item, and moving either side can break the visibility rule.
A small set of visibility forms makes access easy to compare and encourages clear module ownership.

## Examples

### Triggers the lint

```rust
mod pipeline {
    mod parse {
        pub(in crate::pipeline) fn tokenize() {}
    }
}
```

### Use this instead

If `pipeline` owns the operation, define it at that level or expose a purposeful abstraction
there. If the entire crate owns it, say so directly with `pub(crate)`.

```rust
mod pipeline {
    pub(super) fn tokenize() {}

    mod parse {
        // Parsing details remain private to this module.
    }
}
```

## What it skips

Arbitrary ancestor restrictions do not: choosing between relocation and crate-wide ownership requires design context. Macro-generated visibility is ignored because its source belongs to the macro definition.

## When to turn it off

Turn this lint off when your project deliberately follows a different style.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

- [`rlib::unnecessarily_broad_visibility`](../unnecessarily_broad_visibility/README.md) — Finds visibility that is wider than current uses require.
- [`rlib::visibility_required_only_by_tests`](../visibility_required_only_by_tests/README.md) — Finds visibility widened only for tests.
