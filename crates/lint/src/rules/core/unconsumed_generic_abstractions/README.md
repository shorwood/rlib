# `rlib::unconsumed_generic_abstractions`

## Summary

Finds hand-written type parameters on local structs, enums, unions, type aliases, free functions, and inherent methods when every active use supplies the same fully concrete type.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds hand-written type parameters on local structs, enums, unions, type aliases, free functions,
and inherent methods when every active use supplies the same fully concrete type. Parameters
are analyzed independently, so one unused dimension can be specialized without rejecting
meaningful variation in another. Direct calls are resolved through compiler type checking,
including arguments inferred without an explicit turbofish.

Evidence remains open when a parameter is inferred on a named use, forwarded from another
generic context, escapes as a callable value, or involves an alias, projection, opaque type,
trait object, closure, function item, function pointer, unnameable type, dependent bound, or
higher-ranked bound. Trait methods, trait implementations, impl-level type parameters,
generated declarations, unsafe functions, and non-Rust ABIs are excluded. Unrestricted public
APIs in publishable libraries are preserved; binaries and packages marked `publish = false`
are treated as closed. Active test uses count, while disabled `cfg` branches are not compiled.

## Why this matters

A generic parameter with one concrete substitution introduces abstract names,
extra compiled code, and propagation through constructors and impls without supporting an
observed variant. Specializing the declaration keeps the design concrete until another
substitution is genuinely required.

## Examples

### Triggers the lint

```rust
struct Input<T> {
    value: T,
}

fn length<T: AsRef<str>>(input: T) -> usize {
    input.as_ref().len()
}

fn inspect(value: String) -> usize {
    let input = Input { value };
    length(input.value)
}
```

### Use this instead

When both parameters are only ever `String`, express the current design directly:

```rust
struct Input {
    value: String,
}

fn length(input: String) -> usize {
    input.len()
}

fn inspect(value: String) -> usize {
    let input = Input { value };
    length(input.value)
}
```

The lint only sees code compiled in the current run. Parameter names and comments about future
types do not suppress it, while a second concrete type or a caller that stays generic does. It
offers no automatic rewrite because specializing a type parameter can require changes to fields,
constructors, impl blocks, bounds, imports, and inferred call sites.

## What it skips

Active test uses count, while disabled `cfg` branches are not compiled.

## When to turn it off

Turn this lint off when the reported API is deliberate and callers depend on it.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

- [`rlib::single_implementation_traits`](../single_implementation_traits/README.md) — Covers traits that have no current choice of implementation.
