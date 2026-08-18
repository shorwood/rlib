# `rlib::non_defining_module_reexports`

## Summary

Warns about every `use` or `extern crate` declaration whose visibility escapes its containing module, including declarations produced by macros.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Warns about every `use` or `extern crate` declaration whose visibility escapes its containing
module, including declarations produced by macros. This covers `pub`, `pub(crate)`,
`pub(super)`, and `pub(in path)` forms when their resolved scope is broader than the defining
module, for local items, dependency items, renamed imports, and glob imports. Ordinary private
imports, `pub(self)`, and restricted paths that resolve to the defining module are not reexports.

## Why this matters

A reexport gives one declaration multiple plausible homes. Readers and tools can no longer
infer ownership from an import path, module searches must follow façade layers, and an
external dependency can appear to be an API owned by the exporting crate. A visible defining
module provides one truthful standard path. When a crate genuinely needs to own a boundary,
a local trait, newtype, or wrapper makes that ownership explicit instead of borrowing a name.

For example, this façade makes `Parser` appear to belong to the crate root:

## Examples

### Triggers the lint

```rust
mod parser {
    pub struct Parser;
}

pub use parser::Parser;
```

### Use this instead

Expose the defining module and use its standard path:

```rust
pub mod parser {
    pub struct Parser;
}

use parser::Parser;
```

## What it skips

Ordinary private imports, `pub(self)`, and restricted paths that resolve to the defining module are not reexports.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
