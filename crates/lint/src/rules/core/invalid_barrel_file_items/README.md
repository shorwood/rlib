# `rlib::invalid_barrel_file_items`

## Summary

Keeps every physical `mod.rs` and `lib.rs` file focused exclusively on declaring its module tree.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Keeps every physical `mod.rs` and `lib.rs` file focused exclusively on declaring its module
tree. Child module visibility belongs on the module declaration itself; imports,
reexports, macros, inline modules, and implementation code are rejected. Compiler-mandated
proc-macro entry functions remain at the crate root.

## Why this matters

Predictable barrel files let a reader understand a crate's shape without separating useful
declarations from unrelated behavior. They also give an agent one unambiguous place to look
for each concern: barrels describe where code lives, and named files contain that code.

For example, this barrel mixes module structure with implementation:

## Examples

### Triggers the lint

```rust
// mod.rs or lib.rs
mod parser;
use parser::Parser;

fn parse() {}
```

### Use this instead

Move the behavior into a named child file and leave the barrel as a map of the module:

```rust,ignore
// mod.rs or lib.rs
pub mod parser;

// parser.rs
pub struct Parser;
pub fn parse() {}
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

- [`rlib::mixed_module_file_layouts`](../mixed_module_file_layouts/README.md) — Keeps each module under one source root.
