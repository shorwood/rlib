# miette_plain_error_diagnostic_sources

## What it does

Finds causal fields whose local type derives `miette::Diagnostic` but which forward only through
`std::error::Error::source`.

## Why is this bad?

The standard source chain preserves causality but not Miette codes, labels, help, related errors, or
source snippets from the inner diagnostic.

## Example

```rust,ignore
#[source]
source: ParseError,
```

## Use instead

```rust,ignore
#[source]
#[diagnostic_source]
source: ParseError,
```
