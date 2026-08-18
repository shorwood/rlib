# `rlib::needlessly_nested_control_flow`

## Summary

Finds conditional structure that can be flattened through an early exit: useful work in an `else` beside a diverging branch, or a final `if`/`if let` with a non-diverging body that can use `return` or a loop-local `continue` as a guard.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds conditional structure that can be flattened through an early exit: useful work in an
`else` beside a diverging branch, or a final `if`/`if let` with a non-diverging body that can
use `return` or a loop-local `continue` as a guard. Final error handlers that already exit
are not treated as wrapped useful work.

## Why this matters

Guard clauses make exceptional paths short and keep the main operation at the surrounding
indentation level. Retaining an unnecessary branch visually presents the uncommon and common
paths as equally important.

## Examples

### Triggers the lint

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

### Use this instead

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

## What it skips

Final error handlers that already exit are not treated as wrapped useful work.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `control-flow-depth-threshold` | positive integer | `2` | Sets the nesting depth at which the lint starts looking for a guard clause or helper. |

## Known limitations

No known implementation limitations.

## Related lints

- [`rlib::deeply_nested_control_flow`](../deeply_nested_control_flow/README.md) — Limits nesting even when no direct flattening is available.
