# `rlib::misordered_module_declarations`

## Summary

Orders declarations within each hand-written section before local declarations which use them.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Style |
| Default level | `warn` |
| Fix | Automatic |

## What it catches

Orders declarations within each hand-written section before local declarations which use them.
Small modules without section dividers keep their hand-written order. Imports and macro output
are excluded, while types and their immediately adjacent impls move as a unit.

## Why this matters

Reading dependency-first code does not require jumping forward to discover what a local
name means. It also gives modules a deterministic order that can be maintained
automatically as declarations are added.

For example, this function appears before the local type in its signature:

## Examples

### Triggers the lint

```rust
fn open() -> Connection { Connection }
struct Connection;
```

### Use this instead

Declare the dependency before the code that uses it:

```rust
struct Connection;
fn open() -> Connection { Connection }
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

None.
