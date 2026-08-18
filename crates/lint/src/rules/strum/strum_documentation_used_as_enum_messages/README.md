# `rlib::strum_documentation_used_as_enum_messages`

## Summary

Finds `EnumMessage::get_documentation()` values sent directly to user-facing rendering, response, error, or message APIs.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::strum` |
| Cargo feature | `strum` |
| Purpose | Style |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds `EnumMessage::get_documentation()` values sent directly to user-facing rendering, response,
error, or message APIs.

## Why this matters

Rust documentation is developer-facing and may change for clarity. Reusing it as product text turns
documentation edits into observable behavior and bypasses localization.

## Examples

### Triggers the lint

```rust,ignore
show_to_user(failure.get_documentation().unwrap());
```

### Use this instead

Use explicit `#[strum(message = ...)]` metadata, a localization key, or a presentation layer.

```rust,ignore
#[strum(message = "The request could not be completed")]
Rejected
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when your project deliberately follows a different style.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
