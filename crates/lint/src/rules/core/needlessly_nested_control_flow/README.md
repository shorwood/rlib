# needlessly_nested_control_flow

## What it does

Finds conditional structure that can be flattened through an early exit: useful work in an
`else` beside a diverging branch, or a final `if`/`if let` with a non-diverging body that can
use `return` or a loop-local `continue` as a guard. Final error handlers that already exit
are not treated as wrapped useful work.

## Why is this bad?

Guard clauses make exceptional paths short and keep the main operation at the surrounding
indentation level. Retaining an unnecessary branch visually presents the uncommon and common
paths as equally important.

## Example

```rust
fn process(valid: bool) {
    if !valid {
        return;
    } else {
        perform_work();
    }
}
# fn perform_work() {}
```

## Use instead


Remove the branch around the useful work:

```rust
fn process(valid: bool) {
    if !valid {
        return;
    }
    perform_work();
}
# fn perform_work() {}
```
