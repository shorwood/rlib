# `rlib::miette_labels_without_source_code`

## Summary

Finds Miette diagnostic labels that have no local source-code field or forwarded diagnostic source.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::miette` |
| Cargo feature | `miette` |
| Purpose | Correctness |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds Miette diagnostic labels that have no local source-code field or forwarded diagnostic source.

## Why this matters

Labels are offsets into source text. Without source code, reporters cannot render the excerpt or
location the labels are intended to explain.

## Examples

### Triggers the lint

```rust,ignore
#[derive(Debug, thiserror::Error, miette::Diagnostic)]
struct ParseError {
    #[label]
    span: miette::SourceSpan,
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

- [`rlib::miette_source_code_without_labels`](../miette_source_code_without_labels/README.md) — Covers the inverse mismatch: source code with no useful label.
