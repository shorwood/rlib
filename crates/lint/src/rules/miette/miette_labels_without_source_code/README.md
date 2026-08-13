# miette_labels_without_source_code

## What it does

Finds Miette diagnostic labels that have no local source-code field or forwarded diagnostic source.

## Why is this bad?

Labels are offsets into source text. Without source code, reporters cannot render the excerpt or
location the labels are intended to explain.

## Example

```rust,ignore
#[derive(Debug, thiserror::Error, miette::Diagnostic)]
struct ParseError {
    #[label]
    span: miette::SourceSpan,
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
