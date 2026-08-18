# `rlib::discarded_results`

## Summary

Finds standard `Result` values deliberately consumed through a wildcard assignment, an unused underscore-prefixed binding, or an explicit call to `drop`.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Safety |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds standard `Result` values deliberately consumed through a wildcard assignment, an
unused underscore-prefixed binding, or an explicit call to `drop`. An underscore-prefixed
binding that is subsequently read is not a discard. Ordinary unused Results remain covered
by Rust's `unused_must_use` lint, while non-Result values and macro-generated code remain
valid.

## Why this matters

Both forms silence the type's `must_use` warning without stating whether failure should be
propagated, reported, translated, or intentionally ignored. This is especially easy for
generated glue code to introduce when only the happy-path side effect appears relevant.
Once the complete value is gone, later code cannot recover the concrete error or attach the
context needed to diagnose the failed operation.

## Examples

### Triggers the lint

```rust
# fn persist() -> Result<(), std::io::Error> { Ok(()) }
let _ = persist();
drop(persist());
```

### Use this instead

Keep the failure visible at a deliberate policy boundary:

```rust
# fn persist() -> Result<(), std::io::Error> { Ok(()) }
# fn report(_: std::io::Error) {}
if let Err(error) = persist() {
    report(error);
}
```

This lint intentionally provides no automatic fix. Propagation with `?`, contextual error
translation, retry, logging, and deliberate rejection are different choices that require
domain judgment.

## What it skips

An underscore-prefixed binding that is subsequently read is not a discard. Ordinary unused Results remain covered by Rust's `unused_must_use` lint, while non-Result values and macro-generated code remain valid.

## When to turn it off

Turn this lint off only when the risk is handled elsewhere and documented.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

- [`rlib::fallible_values_replaced_with_defaults`](../fallible_values_replaced_with_defaults/README.md) — Covers failures replaced with a default value.
- [`rlib::results_converted_to_options`](../results_converted_to_options/README.md) — Covers failures replaced with `None`.
