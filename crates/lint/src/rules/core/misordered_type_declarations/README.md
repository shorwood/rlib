# misordered_type_declarations

## What it does

Requires local type declarations within each authored section to appear before declarations
that reference them. Small modules without section dividers keep their authored order.
Recursive type groups are kept contiguous in their existing internal order.

## Why is this bad?

Dependency-first type declarations can be read from top to bottom without searching ahead
for each field or variant's definition. Preserving recursive groups avoids inventing an
impossible order for types that depend on one another.

For example, `Request` refers to a type declared later:

## Example

```rust
struct Request(Headers);
struct Headers;
```

## Use instead

Put the dependency before the type that consumes it:

```rust
struct Headers;
struct Request(Headers);
```
