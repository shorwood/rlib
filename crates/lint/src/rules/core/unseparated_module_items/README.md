# unseparated_module_items

## What it does

Requires adjacent authored declarations in a module to have one visually empty line between them.
Imports, `mod` declarations, `extern crate` declarations, and macros are outside this policy and
interrupt adjacency. Documentation and attributes belong to the following declaration, so
separation precedes them.

## Why is this bad?

Dense module declarations hide where one type, implementation, or operation ends and the next
begins. Stable visual boundaries make module ownership easier to scan and edit.

## Example

```rust
struct Report;
impl Report {}
fn render() {}
```

## Use instead

```rust
struct Report;

impl Report {}

fn render() {}
```
