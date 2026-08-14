# documentation_after_attributes

## What it does

Requires declaration documentation to precede every other authored outer attribute. Documentation
comments and explicit `#[doc = ...]` forms are treated as one leading group. The rule covers items,
associated items, fields, and enum variants before attribute macros can replace their source.

## Why is this bad?

Documentation describes the declaration itself, while derives, conditional compilation, lint
levels, and framework attributes modify its compilation. Placing documentation first keeps the
contract immediately visible and gives readers and source tools one deterministic attribute order.

## Example

```rust
#[derive(Debug)]
/// Request state accepted by the transport boundary.
enum RequestState {
    Pending,
    Complete,
}
```

## Use instead

```rust
/// Request state accepted by the transport boundary.
#[derive(Debug)]
enum RequestState {
    Pending,
    Complete,
}
```
