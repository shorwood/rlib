# unconsumed_generic_abstractions

## What it does

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

## Why is this bad?

A generic parameter with one concrete substitution introduces abstract vocabulary,
monomorphized surface, and propagation through constructors and impls without supporting an
observed variant. Specializing the declaration keeps the design concrete until another
substitution is genuinely required.

## Example

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

## Use instead


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

The lint is deliberately compilation-local and structural: parameter names and comments
about future variants do not suppress it, while an observable second substitution or open
generic boundary does. It offers no automatic rewrite because specialization can propagate
through fields, constructors, impl blocks, bounds, imports, and inferred call sites.
