# `rlib::positional_aggregate_fields`

## Summary

Finds tuple structs and tuple-like enum variants with two or more fields.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | API design |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds tuple structs and tuple-like enum variants with two or more fields. Unit forms and
single-field newtypes remain valid because they do not present multiple unnamed roles.

## Why this matters

Naming an aggregate does not make its individual positions self-explanatory. Constructors
and pattern matches still rely on ordering, so readers and agents must repeatedly infer the
same roles. Record fields make the domain names explicit at every use site.

For example, these declarations name the aggregate but not its components:

## Examples

### Triggers the lint

```rust
struct Rename(Span, String);

enum Finding {
    Replacement(Span, String),
}
```

### Use this instead

Use record fields that state what each value means:

```rust
struct Rename {
    span: Span,
    replacement: String,
}

enum Finding {
    Replacement { span: Span, replacement: String },
}
```

## What it skips

Unit forms and single-field newtypes remain valid because they do not present multiple unnamed roles.

## When to turn it off

Turn this lint off when the reported API is deliberate and callers depend on it.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

- [`rlib::bare_tuple_types`](../bare_tuple_types/README.md) — Covers standalone tuple types.
- [`rlib::nested_tuple_types`](../nested_tuple_types/README.md) — Covers tuple types nested inside another type.
