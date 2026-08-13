# missing_section_dividers

## What it does

Requires section dividers when a module contains several independently named concepts or
more declarations than one configured section may hold. A conventional in-source
`#[cfg(test)] mod test` or `mod tests` also gets an explicit divider so production code and
tests have a clear boundary.

## Why is this bad?

A divider is useful when a reader must move between distinct responsibilities. Requiring one
around every declaration creates visual noise, while waiting for a large declaration count
leaves smaller multi-concept modules undifferentiated. This lint recognizes both naming-family
changes and configured size limits, then asks the author to state the real boundaries.

For example, these declarations have no stated family:

## Example

```rust
struct Request;
struct Response;
struct Transport;
struct TransportDecoder;
struct TransportEncoder;
struct TransportRetryPolicy;

#[cfg(test)]
mod tests {
    // ...
}
```

## Use instead

Dividers separate the larger module into meaningful families:

```rust
// -----------------------------------------------------------------------------
// Request: Request and response models
// -----------------------------------------------------------------------------

struct Request;
struct Response;

// -----------------------------------------------------------------------------
// Transport: Transport behavior
// -----------------------------------------------------------------------------

struct Transport;
struct Decoder;
struct Encoder;
struct RetryPolicy;
```
