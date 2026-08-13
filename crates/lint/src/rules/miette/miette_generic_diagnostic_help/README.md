# miette_generic_diagnostic_help

## What it does

Finds static Miette help text that consists only of a configured generic phrase.

## Why is this bad?

Advice such as “try again” or “contact support” does not explain which concrete action can resolve
the reported condition.

## Example

```rust,ignore
#[diagnostic(help("try again"))]
```

## Use instead

```rust,ignore
#[diagnostic(help("set `config_path` to an existing readable file"))]
```
