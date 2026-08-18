# `rlib::miette_misclassified_related_diagnostics`

## Summary

Finds explicit causal field names marked `#[related]` and explicit sibling/aggregate names marked `#[diagnostic_source]`.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::miette` |
| Cargo feature | `miette` |
| Purpose | Correctness |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds explicit causal field names marked `#[related]` and explicit sibling/aggregate names marked
`#[diagnostic_source]`.

## Why this matters

Inverting causal and sibling roles distorts report nesting and makes independent findings appear to
be the root failure, or hides the true source among advisory diagnostics.

## Examples

### Triggers the lint

```rust,ignore
#[related]
source_errors: Vec<ParseError>,
```

### Use this instead

```rust,ignore
#[diagnostic_source]
source: ParseError,
#[related]
findings: Vec<ParseWarning>,
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

None.
