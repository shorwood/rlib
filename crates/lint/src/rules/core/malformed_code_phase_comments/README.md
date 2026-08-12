# malformed_code_phase_comments

## What it does

Validates explanatory comment blocks inside named functions and methods. The first line must
use the configured prefix and sentence-style prose, continuation lines may wrap naturally,
and the block must immediately precede the code it explains. Subsequent blocks require a
separating blank line.

## Why is this bad?

Decorative, empty, or inconsistently placed comments do not explain the code they divide.
Natural prose makes a boundary useful to a reader instead of turning it into a branded or
mechanical line-count escape hatch.

This header has no purpose and is separated from its code:

## Example

```rust
fn prepare() {
    //

    let input = String::new();
}
```

## Use instead


Put concise sentence-style prose directly before the phase:

```rust
fn prepare() {
    // Read and normalize the input state.
    let input = String::new();
}
```
