# `rlib::unseparated_module_items`

## Summary

Requires adjacent hand-written declarations in a module to have one visually empty line between them.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Style |
| Default level | `warn` |
| Fix | Automatic |

## What it catches

Requires adjacent hand-written declarations in a module to have one visually empty line between them.
Imports, `mod` declarations, `extern crate` declarations, and macros are outside this policy and
interrupt adjacency. Documentation and attributes belong to the following declaration, so
separation precedes them.

## Why this matters

Dense module declarations hide where one type, implementation, or operation ends and the next
begins. Stable visual boundaries make module ownership easier to scan and edit.

## Examples

### Triggers the lint

```rust
struct Report;
impl Report {}
fn render() {}
```

### Use this instead

```rust
struct Report;

impl Report {}

fn render() {}
```

## What it skips

Imports, `mod` declarations, `extern crate` declarations, and macros are outside this policy and interrupt adjacency.

## When to turn it off

Turn this lint off when your project deliberately follows a different style.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
