# noncanonical_restricted_visibility

## What it does

Restricts authored definition visibility to four immediately recognizable boundaries:
private, `pub(super)`, `pub(crate)`, and unrestricted `pub`. It rejects `pub(self)`,
`pub(in self)`, `pub(in super)`, `pub(in crate)`, and arbitrary `pub(in path)` syntax on
modules, types, functions, constants, statics, struct and union fields, and associated items.
Import reexports are governed separately by `non_defining_module_reexports`.

Exact aliases receive machine-applicable canonical edits. Arbitrary ancestor restrictions
do not: choosing between relocation and crate-wide ownership requires design context.
Macro-generated visibility is ignored because its source belongs to the macro definition.

## Why is this bad?

`pub(in path)` embeds a particular module ancestry into a declaration's contract. Readers
must resolve that path before understanding who may depend on the item, and moving either
side can invalidate the contract. A deliberately small vocabulary makes reach comparable at
a glance and steers unusually shaped boundaries back toward coherent module ownership.

## Example

```rust
mod pipeline {
    mod parse {
        pub(in crate::pipeline) fn tokenize() {}
    }
}
```

## Use instead

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
