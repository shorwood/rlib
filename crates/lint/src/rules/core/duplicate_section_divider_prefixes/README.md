# `rlib::duplicate_section_divider_prefixes`

## Summary

Warns about repeated section-divider prefixes within the same module.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Correctness |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Warns about repeated section-divider prefixes within the same module. Nested modules have
independent prefix namespaces.

## Why this matters

Repeating a prefix fragments one naming family and makes it unclear which section owns new
declarations. Closely related declarations should remain together.

For example, this module splits the Request family:

## Examples

### Triggers the lint

```rust
// -----------------------------------------------------------------------------
// Request: Request model
// -----------------------------------------------------------------------------
struct Request;

// -----------------------------------------------------------------------------
// Request: Request construction
// -----------------------------------------------------------------------------
struct RequestBuilder;
```

### Use this instead

Keep the family under one divider:

```rust
// -----------------------------------------------------------------------------
// Request: Request model and construction
// -----------------------------------------------------------------------------
struct Request;
struct RequestBuilder;
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off only when the reported behavior is intentional and covered by tests.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

- [`rlib::malformed_section_dividers`](../malformed_section_dividers/README.md) — Checks the shape of each divider.
- [`rlib::mismatched_section_divider_prefixes`](../mismatched_section_divider_prefixes/README.md) — Checks that divider names match their contents.
