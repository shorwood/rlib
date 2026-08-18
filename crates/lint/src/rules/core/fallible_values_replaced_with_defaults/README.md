# `rlib::fallible_values_replaced_with_defaults`

## Summary

Finds standard Result operations that replace the error branch with the success type's `Default` value: `unwrap_or_default`, `unwrap_or(T::default())`, and a direct `unwrap_or_else(|_| T::default())` closure.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Safety |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds standard Result operations that replace the error branch with the success type's
`Default` value: `unwrap_or_default`, `unwrap_or(T::default())`, and a direct
`unwrap_or_else(|_| T::default())` closure. Method and UFCS syntax and type aliases are
recognized semantically. Closures that explicitly discard the error with `let _ = error`
or `drop(error)` are also treated as error-ignoring. Option fallbacks are delegated to
`results_converted_to_options`; custom methods, error-aware closures, explicit matches, and
macro-generated code remain valid.

## Why this matters

A default value looks like an ordinary success value after the call. Downstream code cannot
determine whether empty configuration, an empty collection, zero, or another default was
genuine input or a hidden recovery path. This makes operational failures silently alter
domain behavior and encourages generated code to optimize for compilation rather than an
explicit way of handling failure.

## Examples

### Triggers the lint

```rust
# fn load() -> Result<Vec<String>, std::io::Error> { Ok(Vec::new()) }
let entries = load().unwrap_or_default();
```

### Use this instead

Preserve the error or expose the fallback policy where it happens:

```rust
# fn load() -> Result<Vec<String>, std::io::Error> { Ok(Vec::new()) }
let entries = match load() {
    Ok(entries) => entries,
    Err(error) => {
        eprintln!("using an empty cache after load failure: {error}");
        Vec::new()
    }
};
```

No automatic fix is offered because the correct response may be propagation, translation,
retry, reporting, or a domain-specific fallback rather than `Default`.

## What it skips

Option fallbacks are delegated to `results_converted_to_options`; custom methods, error-aware closures, explicit matches, and macro-generated code remain valid.

## When to turn it off

Turn this lint off only when the risk is handled elsewhere and documented.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

- [`rlib::discarded_results`](../discarded_results/README.md) — Covers results that are ignored outright.
- [`rlib::results_converted_to_options`](../results_converted_to_options/README.md) — Covers failures replaced with `None`.
