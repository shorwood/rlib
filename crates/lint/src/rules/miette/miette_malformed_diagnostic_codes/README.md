# miette_malformed_diagnostic_codes

## What it does

Finds static Miette diagnostic codes that are neither namespaced lowercase Rust paths nor
conventional uppercase letter-number identifiers.

## Why is this bad?

Mixed casing and presentation-like code names tend to drift with user-facing language and make a
crate's machine identifiers incoherent.

## Example

```rust,ignore
#[diagnostic(code(Config::InvalidValue))]
```

## Use instead

```rust,ignore
#[diagnostic(code(config::invalid_value))]
```
