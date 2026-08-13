# miette_duplicate_diagnostic_codes

## What it does

Finds distinct Miette diagnostic types or enum variants that expose the same static diagnostic code.

## Why is this bad?

Diagnostic codes are machine identity used by support, filtering, and telemetry. Reusing one code for
different failures makes those consumers unable to distinguish the contracts.

## Example

```rust,ignore
#[derive(Debug, thiserror::Error, miette::Diagnostic)]
enum ConfigError {
    #[error("missing value")]
    #[diagnostic(code(config::invalid))]
    Missing,
    #[error("invalid value")]
    #[diagnostic(code(config::invalid))]
    Invalid,
}
```

## Use instead

```rust,ignore
#[derive(Debug, thiserror::Error, miette::Diagnostic)]
enum ConfigError {
    #[error("missing value")]
    #[diagnostic(code(config::missing))]
    Missing,
    #[error("invalid value")]
    #[diagnostic(code(config::invalid))]
    Invalid,
}
```
