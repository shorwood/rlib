# miette_missing_diagnostic_codes

## What it does

Finds non-transparent variants without codes in Miette diagnostic enums whose siblings establish a
code policy.

## Why is this bad?

Partially coded families make otherwise comparable failures impossible to filter, track, or identify
in machine-oriented output.

## Example

```rust,ignore
#[derive(Debug, thiserror::Error, miette::Diagnostic)]
enum ConfigError {
    #[error("missing")]
    #[diagnostic(code(config::missing))]
    Missing,
    #[error("invalid")]
    Invalid,
}
```

## Use instead

```rust,ignore
#[error("invalid")]
#[diagnostic(code(config::invalid))]
Invalid,
```
