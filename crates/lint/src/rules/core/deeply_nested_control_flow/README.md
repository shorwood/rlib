# deeply_nested_control_flow

## What it does

Finds authored `if`, `match`, and loop constructs whose nesting exceeds the configured
control-flow depth. `else if` chains remain one decision level, and closures are independent
expressions rather than hidden extensions of their containing function.

## Why is this bad?

Deep control flow multiplies the state a reader must retain and makes individual branches
difficult to name or test. Guard clauses flatten avoidable depth; genuinely nested work
should move behind a named helper.

## Example

```rust
fn process(first: bool, second: bool, third: bool) {
    if first {
        while second {
            if third {}
        }
    }
}
```

## Use instead

Extract the innermost operation or flatten guardable branches before adding another level.

```rust
fn process(first: bool, second: bool, third: bool) {
    if !first {
        return;
    }
    process_enabled(second, third);
}

fn process_enabled(second: bool, third: bool) {
    while second {
        if third {}
    }
}
```
