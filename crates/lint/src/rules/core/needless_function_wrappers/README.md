# needless_function_wrappers

## What it does

Rejects safe Rust functions and inherent methods that do nothing except pass every argument
unchanged to another function in the same crate.

## Why is this bad?

Pure forwarding wrappers obscure the real implementation, scatter documentation and
visibility decisions, and force readers to follow an indirection with no semantic value.

For example, `parse` only repeats `parse_document`'s signature and call:

## Example

```rust
fn parse_document(input: &str) -> usize { input.len() }
fn parse(input: &str) -> usize { parse_document(input) }
```

## Use instead


Remove the wrapper and have callers use the implementation directly:

```rust
fn parse_document(input: &str) -> usize { input.len() }

fn main() {
    let length = parse_document("example");
}
```
