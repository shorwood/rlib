# `rlib::miette_source_code_without_labels`

## Summary

Finds Miette diagnostics that retain source code without labels or nested diagnostics that can focus the report.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::miette` |
| Cargo feature | `miette` |
| Purpose | Correctness |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds Miette diagnostics that retain source code without labels or nested diagnostics that can focus
the report.

## Why this matters

Source storage can be large or cloned frequently. Without diagnostic focus, it adds cost while
providing little more context than the error message.

## Examples

### Triggers the lint

```rust,ignore
struct ParseError {
    #[source_code]
    input: miette::NamedSource<String>,
}
```

### Use this instead

```rust,ignore
struct ParseError {
    #[source_code]
    input: miette::NamedSource<String>,
    #[label]
    span: miette::SourceSpan,
}
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

- [`rlib::miette_labels_without_source_code`](../miette_labels_without_source_code/README.md) — Covers labels that have no source code to point into.
- [`rlib::miette_unfocused_diagnostic_labels`](../miette_unfocused_diagnostic_labels/README.md) — Checks which label is primary when several exist.
