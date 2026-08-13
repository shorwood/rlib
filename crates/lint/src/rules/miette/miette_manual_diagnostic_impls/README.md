# miette_manual_diagnostic_impls

## What it does

Finds hand-written `miette::Diagnostic` implementations containing only defaults, static metadata, or
direct source-field forwarding expressible by Miette derive attributes.

## Why is this bad?

Protocol boilerplate scatters diagnostic policy and obscures implementations that genuinely compute
labels, related errors, localization, or dynamic metadata.

## Example

```rust,ignore
impl miette::Diagnostic for Marker {}
```

## Use instead

```rust,ignore
#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("marker")]
struct Marker;
```
