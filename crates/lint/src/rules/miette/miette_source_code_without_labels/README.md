# miette_source_code_without_labels

## What it does

Finds Miette diagnostics that retain source code without labels or nested diagnostics that can focus
the report.

## Why is this bad?

Source storage can be large or cloned frequently. Without diagnostic focus, it adds cost while
providing little more context than the error message.

## Example

```rust,ignore
struct ParseError {
    #[source_code]
    input: miette::NamedSource<String>,
}
```

## Use instead

```rust,ignore
struct ParseError {
    #[source_code]
    input: miette::NamedSource<String>,
    #[label]
    span: miette::SourceSpan,
}
```
