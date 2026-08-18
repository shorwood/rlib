# `rlib::misordered_test_declarations`

## Summary

Requires each hand-written module to contain at most one conventional in-source `#[cfg(test)] mod test` or `mod tests` block and requires that block to be the final direct declaration.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Style |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Requires each hand-written module to contain at most one conventional in-source
`#[cfg(test)] mod test` or `mod tests` block and requires that block to be the final direct
declaration. Ordering inside the test block is intentionally unconstrained.

## Why this matters

Splitting in-source tests across several modules fragments shared fixtures and makes verification
harder to discover. Placing one test module last gives production declarations and tests a stable
boundary without imposing an artificial order on helpers or individual test functions.

## Examples

### Triggers the lint

```rust
#[cfg(test)]
mod test {}

fn parse() {}

#[cfg(test)]
mod tests {}
```

### Use this instead

```rust
fn parse() {}

// -----------------------------------------------------------------------------
// Tests: In-source tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {}
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when your project deliberately follows a different style.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
