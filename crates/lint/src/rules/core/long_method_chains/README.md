# long_method_chains

## What it does

Finds maximal expressions containing more consecutive method calls than the configured
limit. Associated-function calls do not count, and field access or `?` starts a new chain.

## Why is this bad?

Long fluent expressions hide meaningful transformations behind punctuation and leave no
vocabulary for intermediate states. Named bindings make the data flow inspectable and give
later operations a concept to refer to.

## Example

```rust
fn names(values: &[String]) -> Vec<String> {
    values.iter().filter(|value| !value.is_empty()).cloned().collect()
}
```

## Use instead

Break the expression where an intermediate state has a useful name:

```rust
fn names(values: &[String]) -> Vec<String> {
    let populated = values.iter().filter(|value| !value.is_empty());
    populated.cloned().collect()
}
```

## Configuration

`method-chain-calls-threshold` sets the maximum number of chained calls (default `3`).
