# `rlib::deeply_nested_control_flow`

## Summary

Finds hand-written `if`, `match`, and loop constructs whose nesting exceeds the configured control-flow depth.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds hand-written `if`, `match`, and loop constructs whose nesting exceeds the configured
control-flow depth. `else if` chains remain one decision level, and closures are independent
expressions rather than hidden extensions of their containing function.

## Why this matters

Deep control flow multiplies the state a reader must retain and makes individual branches
difficult to name or test. Guard clauses flatten avoidable depth; genuinely nested work
should move behind a named helper.

## Examples

### Triggers the lint

```rust
fn process(first: bool, second: bool, third: bool) {
    if first {
        while second {
            if third {}
        }
    }
}
```

### Use this instead

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

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `control-flow-depth-threshold` | positive integer | `2` | Sets the deepest allowed nesting for `if`, `match`, and loops. |

## Known limitations

No known implementation limitations.

## Related lints

- [`rlib::needlessly_nested_control_flow`](../needlessly_nested_control_flow/README.md) — Finds nesting that an early exit can remove.
