# `rlib::misordered_type_declarations`

## Summary

Requires local type declarations within each hand-written section to appear before declarations that reference them.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Style |
| Default level | `warn` |
| Fix | Automatic |

## What it catches

Requires local type declarations within each hand-written section to appear before declarations
that reference them. Small modules without section dividers keep their hand-written order.
Recursive type groups are kept contiguous in their existing internal order.

## Why this matters

Dependency-first type declarations can be read from top to bottom without searching ahead
for each field or variant's definition. Preserving recursive groups avoids inventing an
impossible order for types that depend on one another.

For example, `Request` refers to a type declared later:

## Examples

### Triggers the lint

```rust
struct Request(Headers);
struct Headers;
```

### Use this instead

Put the dependency before the type that consumes it:

```rust
struct Headers;
struct Request(Headers);
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
