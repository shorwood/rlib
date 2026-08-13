# thiserror_unreported_error_sources

## What it does

Finds a unique causal-looking field whose resolved local type derives `thiserror::Error` but which
is omitted from the containing derived error's standard source chain.

## Why is this bad?

The nested cause is displayed or stored but unavailable to `std::error::Error::source`, so reporters,
downcasts, and debugging tools lose the causal chain.

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
