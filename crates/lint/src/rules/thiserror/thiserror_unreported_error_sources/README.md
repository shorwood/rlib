# thiserror_unreported_error_sources

## What it does

Finds a single likely cause that is itself a local thiserror type but is omitted from the containing
error's `Error::source` chain.

## Why is this bad?

The nested cause is displayed or stored but unavailable through `Error::source`, so reports and
debugging tools stop at the wrapper instead of reaching the original failure.

## Example

```rust,ignore
#[derive(Debug, thiserror::Error)]
#[error("request failed: {cause}")]
struct RequestError { cause: TransportError }
```

## Use instead

Mark the causal field explicitly, adding `#[from]` only when automatic conversion is also the
intended policy.

```rust,ignore
#[derive(Debug, thiserror::Error)]
#[error("request failed")]
struct RequestError { #[source] cause: TransportError }
```
