# `rlib::thiserror_duplicate_error_messages`

## Summary

Finds distinct variants in a `thiserror::Error` enum with identical static `#[error("...")]` messages.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::thiserror` |
| Cargo feature | `thiserror` |
| Purpose | Correctness |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds distinct variants in a `thiserror::Error` enum with identical static `#[error("...")]`
messages.

## Why this matters

Identical presentation erases the behavioral distinction between typed variants in logs and user
reports, making failures harder to triage and suggested fix harder to explain.

## Examples

### Triggers the lint

```rust,ignore
#[derive(Debug, thiserror::Error)]
enum LookupError {
    #[error("not found")]
    User,
    #[error("not found")]
    Project,
}
```

### Use this instead

Give each behavioral failure useful context while keeping machine identity in the variant or an
explicit diagnostic code.

```rust,ignore
#[derive(Debug, thiserror::Error)]
enum LookupError {
    #[error("user not found")]
    User,
    #[error("project not found")]
    Project,
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

None.
