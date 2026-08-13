# miette_unstable_diagnostic_urls

## What it does

Finds static Miette diagnostic URLs that are relative, insecure, local-development addresses, or
derived from presentation placeholders.

## Why is this bad?

Diagnostic URLs are durable support metadata. Unstable links break tooling and published reports as
messages and development environments change.

## Example

```rust,ignore
#[diagnostic(url("http://localhost/errors/{message}"))]
```

## Use instead

```rust,ignore
#[diagnostic(url("https://docs.example.com/errors/config-invalid"))]
```
