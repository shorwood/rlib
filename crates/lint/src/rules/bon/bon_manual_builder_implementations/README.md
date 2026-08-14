# bon_manual_builder_implementations

## What it does

Finds dedicated `*Builder` structs with a start method, distinct field-named consuming setters
covering their state, and an infallible terminal method producing a nominal value that Bon can
generate. Unrelated consuming fluent methods and scalar-computing terminal methods do not count.

## Why is this bad?

Hand-maintained structural builders repeat setter plumbing and construction protocol while lacking
Bon's compile-time required-member and duplicate-setter guarantees.

## Example

```rust,ignore
struct RequestBuilder { host: String, port: u16 }
// `new`, consuming `host`/`port` setters, and `build` are implemented manually.
```

## Use instead

Use `#[derive(bon::Builder)]` for structural construction, or `#[builder]` on a constructor that owns
validation or normalization.

```rust,ignore
#[derive(bon::Builder)]
struct Request {
    host: String,
    port: u16,
}
```
