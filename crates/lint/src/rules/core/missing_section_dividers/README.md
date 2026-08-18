# `rlib::missing_section_dividers`

## Summary

Requires section dividers when a module contains several independently named concepts or more declarations than one configured section may hold.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Style |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Requires section dividers when a module contains several independently named concepts or
more declarations than one configured section may hold. A conventional in-source
`#[cfg(test)] mod test` or `mod tests` also gets an explicit divider so production code and
tests have a clear boundary. The standard test module body is treated as one declaration
family: this rule owns the divider before the module, while test ordering inside it belongs
to test-specific module layout rules. Integration-test crate roots remain ordinary modules.

## Why this matters

A divider is useful when a reader must move between distinct responsibilities. Requiring one
around every declaration creates visual noise, while waiting for a large declaration count
leaves smaller multi-concept modules undifferentiated. This lint recognizes both naming-family
changes and configured size limits, then asks the author to state the real boundaries.

For example, these declarations have no stated family:

## Examples

### Triggers the lint

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

### Use this instead

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

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when your project deliberately follows a different style.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

- [`rlib::overloaded_declaration_sections`](../overloaded_declaration_sections/README.md) — Finds an existing section that contains too much.
- [`rlib::malformed_section_dividers`](../malformed_section_dividers/README.md) — Checks the dividers already present.
