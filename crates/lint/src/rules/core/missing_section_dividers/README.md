# missing_section_dividers

## What it does

Requires a module-level declaration group to use section dividers when it contains more
distinct declarations than one configured section may hold. Small modules remain readable
without headings. A conventional in-source `#[cfg(test)] mod test` or `mod tests` still gets
an explicit divider so production code and tests have a clear boundary.

## Why is this bad?

A divider is useful when a module is large enough to need navigation. Requiring one around
every type creates visual noise and obscures the few boundaries that matter. This lint waits
until a declaration group exceeds the configured section size, then asks the author to find
the real conceptual boundaries.

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
