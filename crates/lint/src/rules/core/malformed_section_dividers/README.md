# `rlib::malformed_section_dividers`

## Summary

Validates section-divider content and rendered width against the configured template.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Style |
| Default level | `warn` |
| Fix | Automatic |

## What it catches

Validates section-divider content and rendered width against the configured template.
Content names a PascalCase abstraction and its sentence-style responsibility, separated by a
colon. The responsibility starts with a capital letter while preserving hand-written proper nouns,
acronyms, and hyphenated technical terms. A bare abstraction name is incomplete because it does
not explain why the declarations belong together.

## Why this matters

Inconsistent syntax weakens dividers as navigation landmarks and makes generated edits
unpredictable. A name without a responsibility merely repeats the declaration below it, while
empty or overlong sections hide rather than clarify source structure.

For example, this divider has nonstandard spacing and description casing:

## Examples

### Triggers the lint

```rust
// -----------------------------------------------------------------------------
// Request : request handling
// -----------------------------------------------------------------------------
struct Request;
```

### Use this instead

Render the same content consistently:

```rust
// -----------------------------------------------------------------------------
// Request: Request handling
// -----------------------------------------------------------------------------
struct Request;
```

## What it skips

A bare abstraction name is incomplete because it does not explain why the declarations belong together.

## When to turn it off

Turn this lint off when your project deliberately follows a different style.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

- [`rlib::missing_section_dividers`](../missing_section_dividers/README.md) — Finds modules that need dividers.
- [`rlib::duplicate_section_divider_prefixes`](../duplicate_section_divider_prefixes/README.md) — Finds repeated divider names.
