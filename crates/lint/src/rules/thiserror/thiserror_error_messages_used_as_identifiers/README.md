# thiserror_error_messages_used_as_identifiers

## What it does

Finds equality, prefix, suffix, and substring decisions made from standard `ToString` output of a
value whose type derives `thiserror::Error`.

## Why is this bad?

Error messages are presentation. Treating them as machine identity turns wording and punctuation
changes into behavioral or protocol breaks while discarding the stable typed variant.

## Example

```rust,ignore
fn is_missing(error: LookupError) -> bool {
    error.to_string() == "not found"
}
```

## Use instead

Match the typed error or introduce an explicit stable code when identity must cross a dynamic
boundary.

```rust,ignore
fn is_missing(error: LookupError) -> bool {
    matches!(error, LookupError::Missing)
}
```
