# `rlib::function_local_items`

## Summary

Warns about hand-written item declarations inside functions, methods, closures, and their nested blocks.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Warns about hand-written item declarations inside functions, methods, closures, and their nested blocks.
This includes local types and impls, helper functions, constants and statics, imports, modules, and
macro definitions. Items produced by macro expansion are ignored.

## Why this matters

Local items mix declarations with executable control flow, conceal reusable helpers, and
make their ownership harder to discover. Module or associated scope gives declarations a stable,
searchable home.

## Examples

### Triggers the lint

```rust
fn render() {
    struct Renderer;
    impl Renderer {}
}
```

### Use this instead

```rust
struct Renderer;

impl Renderer {}

fn render() {}
```

## What it skips

Items produced by macro expansion are ignored.

## When to turn it off

Turn this lint off when the reported API is deliberate and callers depend on it.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
