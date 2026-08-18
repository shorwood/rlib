# `rlib::non_adjacent_struct_impls`

## Summary

Checks that all direct impl blocks declared in the same module as a struct form one group immediately after the struct.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Automatic |

## What it catches

Checks that all direct impl blocks declared in the same module as a struct form one group
immediately after the struct. The separate file-colocation rule handles impls moved to another
physical file; impls inside a nested module in the same file are outside this ordering rule.

## Why this matters

Keeping a struct and its behavior together makes the type easier to understand without
searching through the rest of the module.

For example, this helper splits the struct from its implementation:

## Examples

### Triggers the lint

```rust
struct Cache;
fn cache_directory() -> &'static str { "/tmp" }
impl Cache {
    fn clear(&mut self) {}
}
```

### Use this instead

Keep the complete impl group directly after the definition:

```rust
struct Cache;
impl Cache {
    fn clear(&mut self) {}
}
fn cache_directory() -> &'static str { "/tmp" }
```

## What it skips

The separate file-colocation rule handles impls moved to another physical file; impls inside a nested module in the same file are outside this ordering rule.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

- [`rlib::cross_file_struct_impls`](../cross_file_struct_impls/README.md) — Covers implementations moved to another file.
- [`rlib::non_adjacent_extension_trait_impls`](../non_adjacent_extension_trait_impls/README.md) — Applies the same grouping rule to extension traits.
