# invalid_barrel_file_items

## What it does

Keeps every physical `mod.rs` and `lib.rs` file focused exclusively on declaring its module
tree. Child module visibility belongs on the module declaration itself; imports,
reexports, macros, inline modules, and implementation code are rejected. Compiler-mandated
proc-macro entry functions remain at the crate root.

## Why is this bad?

Predictable barrel files let a reader understand a crate's shape without separating useful
declarations from unrelated behavior. They also give an agent one unambiguous place to look
for each concern: barrels describe where code lives, and named files contain that code.

For example, this barrel mixes module structure with implementation:

## Example

```rust
// mod.rs or lib.rs
mod parser;
use parser::Parser;

fn parse() {}
```

## Use instead

Move the behavior into a named child file and leave the barrel as a map of the module:

```rust,ignore
// mod.rs or lib.rs
pub mod parser;

// parser.rs
pub struct Parser;
pub fn parse() {}
```
