# misordered_test_declarations

## What it does

Requires each authored module to contain at most one conventional in-source
`#[cfg(test)] mod test` or `mod tests` block and requires that block to be the final direct
declaration. Ordering inside the test block is intentionally unconstrained.

## Why is this bad?

Splitting in-source tests across several modules fragments shared fixtures and makes verification
harder to discover. Placing one test module last gives production declarations and tests a stable
boundary without imposing an artificial order on helpers or individual test functions.

## Example

```rust
#[cfg(test)]
mod test {}

fn parse() {}

#[cfg(test)]
mod tests {}
```

## Use instead

```rust
fn parse() {}

// -----------------------------------------------------------------------------
// Tests: In-source tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {}
```
