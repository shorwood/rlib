# `rlib::boolean_function_arguments`

## Summary

Finds direct boolean function and method parameters.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds direct boolean function and method parameters. One boolean is accepted only for an
exact setter shape such as `set_enabled(enabled: bool)`, with no other non-receiver
parameters; two or more are always rejected.
Boolean returns, predicate callbacks, and wrapped state such as `Option<bool>` are outside
the rule; boolean-result naming is covered separately.

## Why this matters

A boolean argument hides a choice at its call site. Literals such as `true` and `false` do
not explain the policy they select, and multiple flags can be reordered without a type
error. Adding another boolean is an easy compiling change for an agent but steadily erodes
the API's names.

This call makes neither decision reviewable:

## Examples

### Triggers the lint

```rust
fn render(document: &Document, minify: bool, include_metadata: bool) {}

render(&document, true, false);
```

### Use this instead

Name the policy through an enum or options type:

```rust
struct RenderOptions {
    minify: bool,
    include_metadata: bool,
}

fn render(document: &Document, options: RenderOptions) {}
```

## What it skips

Boolean returns, predicate callbacks, and wrapped state such as `Option<bool>` are outside the rule.

## When to turn it off

Turn this lint off when the reported API is deliberate and callers depend on it.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

- [`rlib::bool_fields_without_predicate_prefix`](../bool_fields_without_predicate_prefix/README.md) — Covers boolean values stored in named fields.
- [`rlib::bool_returning_functions_without_predicate_prefix`](../bool_returning_functions_without_predicate_prefix/README.md) — Covers predicate naming for boolean results.
- [`rlib::ambiguous_primitive_parameters`](../ambiguous_primitive_parameters/README.md) — Covers repeated primitive types with different meanings.
