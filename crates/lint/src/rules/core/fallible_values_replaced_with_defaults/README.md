# fallible_values_replaced_with_defaults

## What it does

Finds standard Result operations that replace the error branch with the success type's
`Default` value: `unwrap_or_default`, `unwrap_or(T::default())`, and a direct
`unwrap_or_else(|_| T::default())` closure. Method and UFCS syntax and type aliases are
recognized semantically. Option fallbacks are delegated to
`results_converted_to_options`; custom methods, error-aware closures, explicit matches, and
macro-generated code remain valid.

## Why is this bad?

A default value looks like an ordinary success value after the call. Downstream code cannot
determine whether empty configuration, an empty collection, zero, or another default was
genuine input or a hidden recovery path. This makes operational failures silently alter
domain behavior and encourages generated code to optimize for compilation rather than an
explicit failure contract.

## Example

```rust
# fn load() -> Result<Vec<String>, std::io::Error> { Ok(Vec::new()) }
let entries = load().unwrap_or_default();
```

## Use instead

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
