# non_defining_module_reexports

## What it does

Rejects every `use` or `extern crate` declaration whose visibility escapes its containing
module, including declarations produced by macros. This covers `pub`, `pub(crate)`,
`pub(super)`, and `pub(in path)` forms for local items, dependency items, renamed imports,
and glob imports. Ordinary private imports and the explicitly private `pub(self)` form are
not reexports.

## Why is this bad?

A reexport gives one declaration multiple plausible homes. Readers and tools can no longer
infer ownership from an import path, module searches must follow façade layers, and an
external dependency can appear to be an API owned by the exporting crate. A visible defining
module provides one truthful canonical path. When a crate genuinely needs to own a boundary,
a local trait, newtype, or wrapper makes that ownership explicit instead of borrowing a name.

For example, this façade makes `Parser` appear to belong to the crate root:

## Example

```rust
mod parser {
    pub struct Parser;
}

pub use parser::Parser;
```

## Use instead


Expose the defining module and use its canonical path:

```rust
pub mod parser {
    pub struct Parser;
}

use parser::Parser;
```
