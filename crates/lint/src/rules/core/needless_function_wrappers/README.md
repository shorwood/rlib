# `rlib::needless_function_wrappers`

## Summary

Warns about safe Rust functions and inherent methods that do nothing except pass every argument unchanged to another function in the same crate.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Warns about safe Rust functions and inherent methods that do nothing except pass every argument
unchanged to another function in the same crate.
Unsafe blocks are behavioral boundaries: a safe function that encapsulates an unsafe call is not
treated as transparent forwarding.

## Why this matters

Pure forwarding wrappers obscure the real implementation, scatter documentation and
visibility decisions, and force readers to follow an indirection with no behavioral value.

For example, `parse` only repeats `parse_document`'s signature and call:

## Examples

### Triggers the lint

```rust
fn parse_document(input: &str) -> usize { input.len() }
fn parse(input: &str) -> usize { parse_document(input) }
```

### Use this instead

Remove the wrapper and have callers use the implementation directly:

```rust
fn parse_document(input: &str) -> usize { input.len() }

fn main() {
    let length = parse_document("example");
}
```

## What it skips

Unsafe blocks are behavioral boundaries: a safe function that encapsulates an unsafe call is not treated as transparent forwarding.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

- [`rlib::needless_delegating_types`](../needless_delegating_types/README.md) — Covers types whose methods only forward calls.
