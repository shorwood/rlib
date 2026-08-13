# miette_unfocused_diagnostic_labels

## What it does

Finds Miette diagnostics with multiple labels but no explicitly primary label.

## Why is this bad?

Readers and reporters need a clear causal location. Field order is not a stable or intentional way
to distinguish the main failure from supporting spans.

## Example

```rust,ignore
#[label]
first: miette::SourceSpan,
#[label]
second: miette::SourceSpan,
```

## Use instead

```rust,ignore
#[label(primary)]
first: miette::SourceSpan,
#[label]
second: miette::SourceSpan,
```
