# `rlib::bool_returning_functions_without_predicate_prefix`

## Summary

Checks that functions and methods with boolean results use predicate-style names.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Style |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Checks functions, inherent methods, and local trait methods whose return value is `bool`, directly
or through the success value of standard `Option` and `Result` containers. The configured property
prefix must introduce a nonempty predicate phrase. Canonical query roots may also be used exactly or
followed by another underscore-separated phrase.

The default property prefixes are `is_`, `has_`, and `should_`. The default callable-only query
roots are `all`, `any`, `contains`, `ends_with`, `exists`, `starts_with`, and `try_exists`.

## Why this matters

A predicate name tells callers that an operation answers a yes-or-no question. Names that look like
values or commands hide that contract and make conditions harder to read.

## Examples

### Triggers the lint

```rust
fn matching_context() -> Result<bool, LookupError> {
    Ok(true)
}
```

### Use this instead

```rust
fn is_matching_context() -> Result<bool, LookupError> {
    Ok(true)
}
```

## What it skips

Closures, foreign functions, generated code, trait implementations, custom wrappers, collections,
and booleans outside the success path are not checked. A non-async function returning a future is
also outside the rule.

## When to turn it off

Turn this lint off when a callable must preserve an externally defined name that the project cannot
change.

## Settings

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `boolean-predicate-prefixes` | string list | built-in list | Sets property prefixes shared with boolean field naming. Use `".."` to keep the built-in entries. |
| `boolean-query-roots` | string list | built-in list | Sets canonical query roots accepted only for callables. Use `".."` to keep the built-in entries. |

## Known limitations

Only standard `Option` and `Result` success containers are followed.

## Related lints

- [`rlib::bool_fields_without_predicate_prefix`](../bool_fields_without_predicate_prefix/README.md) — Applies the shared property-prefix policy to boolean struct fields.
- [`rlib::boolean_function_arguments`](../boolean_function_arguments/README.md) — Covers boolean values passed into callables.
