# `rlib::thiserror_non_send_sync_public_errors`

## Summary

Finds public thiserror types sent through public channel APIs even though one of their fields is known to be limited to the current thread.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::thiserror` |
| Cargo feature | `thiserror` |
| Purpose | Safety |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds public thiserror types sent through public channel APIs even though one of their fields is
known to be limited to the current thread.

## Why this matters

The interface promises that the error can cross threads, but its stored data breaks that promise.
The compiler error often appears at a distant call site instead of beside the field that caused it.

## Examples

### Triggers the lint

```rust,ignore
#[derive(Debug, thiserror::Error)]
#[error("failed: {context}")]
pub struct LocalError { context: std::rc::Rc<String> }

pub fn error_sender() -> std::sync::mpsc::Sender<LocalError> { todo!() }
```

### Use this instead

Use a thread-safe representation such as `Arc`, or narrow the API to an explicitly local boundary.

```rust,ignore
#[derive(Debug, thiserror::Error)]
#[error("failed: {context}")]
pub struct SharedError { context: std::sync::Arc<String> }
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
