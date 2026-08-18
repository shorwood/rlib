# `rlib::miette_unstable_diagnostic_urls`

## Summary

Finds static Miette diagnostic URLs that are relative, insecure, local-development addresses, or derived from presentation placeholders.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::miette` |
| Cargo feature | `miette` |
| Purpose | Correctness |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds static Miette diagnostic URLs that are relative, insecure, local-development addresses, or
derived from presentation placeholders.

## Why this matters

Diagnostic URLs are durable support metadata. Unstable links break tooling and published reports as
messages and development environments change.

## Examples

### Triggers the lint

```rust,ignore
#[diagnostic(url("http://localhost/errors/{message}"))]
```

### Use this instead

```rust,ignore
#[diagnostic(url("https://docs.example.com/errors/config-invalid"))]
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
