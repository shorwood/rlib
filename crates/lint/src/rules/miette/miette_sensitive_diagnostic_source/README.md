# `rlib::miette_sensitive_diagnostic_source`

## Summary

Finds strongly named credential or private-content fields exposed through `#[source_code]`.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::miette` |
| Cargo feature | `miette` |
| Purpose | Safety |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds strongly named credential or private-content fields exposed through `#[source_code]`.

## Why this matters

Miette reporters can print excerpts and surrounding text to terminals, logs, or serialized reports,
disclosing secrets far from the diagnostic declaration.

## Examples

### Triggers the lint

```rust,ignore
#[source_code]
request_body: miette::NamedSource<String>,
```

### Use this instead

```rust,ignore
#[source_code]
redacted_request: miette::NamedSource<String>,
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off only when the risk is handled elsewhere and documented.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
