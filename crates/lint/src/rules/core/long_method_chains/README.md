# `rlib::long_method_chains`

## Summary

Finds maximal expressions containing more consecutive method calls than the configured limit.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds maximal expressions containing more consecutive method calls than the configured
limit. Associated-function calls do not count, and field access or `?` starts a new chain.

## Why this matters

Long fluent expressions hide meaningful transformations behind punctuation and leave no
names for intermediate states. Named bindings make the data flow inspectable and give
later operations a concept to refer to.

## Examples

### Triggers the lint

```rust
fn names(values: &[String]) -> Vec<String> {
    values.iter().filter(|value| !value.is_empty()).cloned().collect()
}
```

### Use this instead

Break the expression where an intermediate state has a useful name:

```rust
fn names(values: &[String]) -> Vec<String> {
    let populated = values.iter().filter(|value| !value.is_empty());
    populated.cloned().collect()
}
```

## What it skips

Associated-function calls do not count, and field access or `?` starts a new chain. Calls
introduced by compiler or attribute-macro desugaring are ignored when the attributed source span
does not contain authored receiver syntax.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `method-chain-calls-threshold` | positive integer | `3` | Sets the largest number of chained method calls allowed without a named intermediate value. |

## Known limitations

No known implementation limitations.

## Related lints

None.
