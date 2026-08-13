# miette_misclassified_related_diagnostics

## What it does

Finds explicit causal field names marked `#[related]` and explicit sibling/aggregate names marked
`#[diagnostic_source]`.

## Why is this bad?

Inverting causal and sibling roles distorts report nesting and makes independent findings appear to
be the root failure, or hides the true source among advisory diagnostics.

## Example

```rust,ignore
#[related]
source_errors: Vec<ParseError>,
```

## Use instead

```rust,ignore
#[diagnostic_source]
source: ParseError,
#[related]
findings: Vec<ParseWarning>,
```
