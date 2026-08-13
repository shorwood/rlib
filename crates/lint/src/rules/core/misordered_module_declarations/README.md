# misordered_module_declarations

## What it does

Orders declarations within each authored section before local declarations which use them.
Small modules without section dividers keep their authored order. Imports and macro output
are excluded, while types and their immediately adjacent impls move as a unit.

## Why is this bad?

Reading dependency-first code does not require jumping forward to discover what a local
name means. It also gives modules a deterministic order that can be maintained
automatically as declarations are added.

For example, this function appears before the local type in its signature:

## Example

```rust
fn open() -> Connection { Connection }
struct Connection;
```

## Use instead


Declare the dependency before the code that uses it:

```rust
struct Connection;
fn open() -> Connection { Connection }
```
