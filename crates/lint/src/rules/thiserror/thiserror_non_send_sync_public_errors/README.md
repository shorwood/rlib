# thiserror_non_send_sync_public_errors

## What it does

Finds public derived thiserror types exposed through public channel APIs while a concrete field type
is known to prevent `Send` or `Sync`.

## Why is this bad?

The interface advertises a cross-thread transport for an error value that cannot satisfy the
expected auto traits. This usually surfaces later as an integration failure far from the field that
caused it.

## Example

```rust,ignore
#[derive(Debug, thiserror::Error)]
#[error("failed: {context}")]
pub struct LocalError { context: std::rc::Rc<String> }

pub fn error_sender() -> std::sync::mpsc::Sender<LocalError> { todo!() }
```

## Use instead

Use a thread-safe representation such as `Arc`, or narrow the API to an explicitly local boundary.

```rust,ignore
#[derive(Debug, thiserror::Error)]
#[error("failed: {context}")]
pub struct SharedError { context: std::sync::Arc<String> }
```
