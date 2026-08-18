# `rlib::thiserror_error_messages_used_as_identifiers`

## Summary

Finds equality, prefix, suffix, and substring decisions made from standard `ToString` output of a value whose type derives `thiserror::Error`.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::thiserror` |
| Cargo feature | `thiserror` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds equality, prefix, suffix, and substring decisions made from standard `ToString` output of a
value whose type derives `thiserror::Error`.

## Why this matters

Error messages are presentation. Treating them as machine identity turns wording and punctuation
changes into behavioral or compatibility breaks while discarding the stable typed variant.

## Examples

### Triggers the lint

```rust,ignore
fn is_missing(error: LookupError) -> bool {
    error.to_string() == "not found"
}
```

### Use this instead

Match the typed error or introduce an explicit stable code when identity must cross a dynamic
boundary.

```rust,ignore
fn is_missing(error: LookupError) -> bool {
    matches!(error, LookupError::Missing)
}
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
