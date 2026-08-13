# missing_code_phase_comments

## What it does

Finds named functions and methods whose direct block surface exceeds the configured limit
without being divided into short, explanatory phases. Nested hand-written blocks are measured
independently so their implementation does not inflate the containing phase. Declarative
struct literals and literal-only match mappings count as one operation regardless of their
formatting. The physical line limit and comment prefix are configurable through the shared
`function_structure` table.

## Why is this bad?

A long uninterrupted sequence forces readers to reconstruct where preparation ends and the
next operation begins. Natural prose comments provide navigation when the work remains
inherently sequential. Treating control flow as self-explanatory hides mixed workflows,
while counting nested bodies against their parent reports the same complexity twice.

For a three-line limit, this run has no named phases:

## Example

```rust
fn prepare() {
    let input = String::new();
    let trimmed = input.trim();
    let length = trimmed.len();
    let empty = trimmed.is_empty();
}
```

## Use instead


Explain every phase, including the first, and keep each below the configured limit:

```rust
fn prepare() {
    // Read and normalize the input before deriving its properties.
    let input = String::new();
    let trimmed = input.trim();

    // Derive the properties consumed by the caller.
    let length = trimmed.len();
    let empty = trimmed.is_empty();
}
```
