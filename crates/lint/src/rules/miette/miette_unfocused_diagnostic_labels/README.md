# `rlib::miette_unfocused_diagnostic_labels`

## Summary

Finds Miette diagnostics with multiple labels but no explicitly primary label.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::miette` |
| Cargo feature | `miette` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds Miette diagnostics with multiple labels but no explicitly primary label.

## Why this matters

Readers and reporters need a clear causal location. Field order is not a stable or intentional way
to distinguish the main failure from supporting spans.

## Examples

### Triggers the lint

```rust,ignore
#[label]
first: miette::SourceSpan,
#[label]
second: miette::SourceSpan,
```

### Use this instead

```rust,ignore
#[label(primary)]
first: miette::SourceSpan,
#[label]
second: miette::SourceSpan,
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

- [`rlib::miette_source_code_without_labels`](../miette_source_code_without_labels/README.md) — Checks whether retained source code has useful labels.
