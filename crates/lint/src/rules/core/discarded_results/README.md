# discarded_results

## What it does

Finds standard `Result` values deliberately consumed through a wildcard assignment, an
underscore-prefixed binding, or an explicit call to `drop`. Ordinary unused Results remain
covered by Rust's `unused_must_use` lint, while non-Result values and macro-generated code
remain valid.

## Why is this bad?

Both forms silence the type's must-use contract without stating whether failure should be
propagated, reported, translated, or intentionally ignored. This is especially easy for
generated glue code to introduce when only the happy-path side effect appears relevant.
Once the complete value is gone, later code cannot recover the concrete error or attach the
context needed to diagnose the failed operation.

## Example

```rust
# fn persist() -> Result<(), std::io::Error> { Ok(()) }
let _ = persist();
drop(persist());
```

## Use instead


Keep the failure visible at a deliberate policy boundary:

```rust
# fn persist() -> Result<(), std::io::Error> { Ok(()) }
# fn report(_: std::io::Error) {}
if let Err(error) = persist() {
    report(error);
}
```

This lint intentionally provides no automatic fix. Propagation with `?`, contextual error
translation, retry, logging, and deliberate rejection are different contracts that require
domain judgment.
