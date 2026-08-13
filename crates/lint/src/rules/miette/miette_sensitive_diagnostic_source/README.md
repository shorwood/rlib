# miette_sensitive_diagnostic_source

## What it does

Finds strongly named credential or private-content fields exposed through `#[source_code]`.

## Why is this bad?

Miette reporters can print excerpts and surrounding text to terminals, logs, or serialized reports,
disclosing secrets far from the diagnostic declaration.

## Example

```rust,ignore
#[source_code]
request_body: miette::NamedSource<String>,
```

## Use instead

```rust,ignore
#[source_code]
redacted_request: miette::NamedSource<String>,
```
