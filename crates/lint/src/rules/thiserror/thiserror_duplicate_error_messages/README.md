# thiserror_duplicate_error_messages

## What it does

Finds distinct variants in a `thiserror::Error` enum with identical static `#[error("...")]`
messages.

## Why is this bad?

Identical presentation erases the behavioral distinction between typed variants in logs and user
reports, making failures harder to triage and suggested fix harder to explain.

## Example

```rust,ignore
#[derive(Debug, thiserror::Error)]
enum LookupError {
    #[error("not found")]
    User,
    #[error("not found")]
    Project,
}
```

## Use instead

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
