# `rlib::bool_fields_without_predicate_prefix`

## Summary

Checks that named boolean fields in structs use `is_<predicate>` or `has_<predicate>`.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Style |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Checks that named boolean fields in structs use `is_<predicate>` or `has_<predicate>`. The prefix
must be followed by a nonempty, ordinarily formed predicate phrase.

## Why this matters

A predicate prefix makes the meaning of a boolean field clear at call sites. Without one,
the field can read like a command, an event, or an arbitrary value instead of a yes-or-no
property.

For example, these field names do not communicate that they are predicates:

## Examples

### Triggers the lint

```rust
struct Window {
    active: bool,
    children: bool,
}
```

### Use this instead

Prefixing them makes their role explicit wherever the fields are read:

```rust
struct Window {
    is_active: bool,
    has_children: bool,
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

- [`rlib::boolean_function_arguments`](../boolean_function_arguments/README.md) — Covers boolean values passed as function arguments.
