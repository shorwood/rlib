# `rlib::documentation_after_attributes`

## Summary

Requires declaration documentation to precede every other hand-written outer attribute.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Style |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Requires declaration documentation to precede every other hand-written outer attribute. Documentation
comments and explicit `#[doc = ...]` forms are treated as one leading group. The rule covers items,
associated items, fields, and enum variants before attribute macros can replace their source.

## Why this matters

Documentation describes the declaration itself, while derives, conditional compilation, lint
levels, and framework attributes change how it is compiled. Placing documentation first makes the
description easy to find and gives readers and source tools one predictable attribute order.

## Examples

### Triggers the lint

```rust
#[derive(Debug)]
/// Request state accepted by the transport boundary.
enum RequestState {
    Pending,
    Complete,
}
```

### Use this instead

```rust
/// Request state accepted by the transport boundary.
#[derive(Debug)]
enum RequestState {
    Pending,
    Complete,
}
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

- [`rlib::undocumented_items`](../undocumented_items/README.md) — Requires the documentation itself to be present.
