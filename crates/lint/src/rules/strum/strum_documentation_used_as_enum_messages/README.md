# strum_documentation_used_as_enum_messages

## What it does

Finds `EnumMessage::get_documentation()` values sent directly to user-facing rendering, response,
error, or message APIs.

## Why is this bad?

Rust documentation is developer-facing and may change for clarity. Reusing it as product text turns
documentation edits into observable behavior and bypasses localization.

## Example

```rust,ignore
show_to_user(failure.get_documentation().unwrap());
```

## Use instead

Use explicit `#[strum(message = ...)]` metadata, a localization key, or a presentation layer.

```rust,ignore
#[strum(message = "The request could not be completed")]
Rejected
```
