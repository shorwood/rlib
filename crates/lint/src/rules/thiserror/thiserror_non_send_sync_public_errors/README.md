# thiserror_non_send_sync_public_errors

## What it does

Finds public thiserror types sent through public channel APIs even though one of their fields is
known to be limited to the current thread.

## Why is this bad?

The interface promises that the error can cross threads, but its stored data breaks that promise.
The compiler error often appears at a distant call site instead of beside the field that caused it.

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
