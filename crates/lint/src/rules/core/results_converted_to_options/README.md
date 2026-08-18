# `rlib::results_converted_to_options`

## Summary

Finds standard Result operations that directly replace failure with Option absence: `ok`, Option-valued `map_or` and `map_or_else`, and Option defaults supplied through `unwrap_or`, `unwrap_or_else`, or `unwrap_or_default`.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Safety |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds standard Result operations that directly replace failure with Option absence:
`ok`, Option-valued `map_or` and `map_or_else`, and Option defaults supplied through
`unwrap_or`, `unwrap_or_else`, or `unwrap_or_default`. Method and UFCS syntax and type
aliases are resolved semantically. Custom same-named methods, explicit matches,
error-aware closures, and macro-generated code remain valid.

## Why this matters

These operations compress two distinct states into `None`: domain absence and operational
failure. Their concise fallback syntax hides both the erased error type and the location
where the application chose to stop treating failure as failure. Agents can then propagate
the optional value through unrelated code without enough context to recover the original
policy.

## Examples

### Triggers the lint

```rust
# fn load() -> Result<String, std::io::Error> { Ok(String::new()) }
let cached = load().ok();
```

### Use this instead

Keep the Result intact, or make the translation reviewable in hand-written control flow:

```rust
# fn load() -> Result<String, std::io::Error> { Ok(String::new()) }
let cached = match load() {
    Ok(value) => Some(value),
    Err(error) => {
        eprintln!("cache unavailable: {error}");
        None
    }
};
```

No automatic fix is offered because propagation, reporting, and intentional absence have
different types and ownership requirements.

## What it skips

Custom same-named methods, explicit matches, error-aware closures, and macro-generated code remain valid.

## When to turn it off

Turn this lint off only when the risk is handled elsewhere and documented.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

- [`rlib::discarded_results`](../discarded_results/README.md) — Covers results that are ignored outright.
- [`rlib::fallible_values_replaced_with_defaults`](../fallible_values_replaced_with_defaults/README.md) — Covers failures replaced with a default value.
