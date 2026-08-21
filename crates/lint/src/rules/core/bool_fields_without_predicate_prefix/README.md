# `rlib::bool_fields_without_predicate_prefix`

## Summary

Checks that named boolean fields in structs use a configured predicate prefix.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Style |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Checks that named boolean fields in structs use a configured predicate prefix. The default prefixes
are `is_`, `has_`, and `should_`, and each must be followed by a nonempty, ordinarily formed
predicate phrase.

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
    should_close: bool,
}
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when your project deliberately follows a different style.

## Settings

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `boolean-predicate-prefixes` | string list | built-in list | Sets property prefixes shared with boolean-result callable naming. Use `".."` to keep the built-in entries. |

## Known limitations

No known implementation limitations.

## Related lints

- [`rlib::bool_returning_functions_without_predicate_prefix`](../bool_returning_functions_without_predicate_prefix/README.md) — Applies the shared prefix policy to boolean-result functions and methods.
- [`rlib::boolean_function_arguments`](../boolean_function_arguments/README.md) — Covers boolean values passed as function arguments.
