# `rlib::miette_plain_error_diagnostic_sources`

## Summary

Finds causal fields whose local type derives `miette::Diagnostic` but which forward only through `std::error::Error::source`.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::miette` |
| Cargo feature | `miette` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds causal fields whose local type derives `miette::Diagnostic` but which forward only through
`std::error::Error::source`.

## Why this matters

The standard source chain preserves causality but not Miette codes, labels, help, related errors, or
source snippets from the inner diagnostic.

## Examples

### Triggers the lint

```rust,ignore
#[source]
source: ParseError,
```

### Use this instead

```rust,ignore
#[source]
#[diagnostic_source]
source: ParseError,
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
