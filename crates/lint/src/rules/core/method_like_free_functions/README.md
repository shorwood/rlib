# `rlib::method_like_free_functions`

## Summary

Checks for free functions whose first parameter can be the receiver of an inherent method on a struct defined in the same module.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | API design |
| Default level | `warn` |
| Fix | Automatic |

## What it catches

Checks for free functions whose first parameter can be the receiver of an inherent method
on a struct defined in the same module.
Automatic migrations move generic parameters only when the resolved receiver arguments actually
use them; name substrings and other ambiguous syntax receive manual guidance.

## Why this matters

Keeping behavior on the type it belongs to makes that behavior easier to discover and keeps
the module's free-function namespace focused on operations that do not belong to one type.

For example, this free function behaves like part of `Document`'s interface:

## Examples

### Triggers the lint

```rust
struct Document;

fn render(document: &Document) {}
```

### Use this instead

Making the first parameter the receiver puts the operation where callers expect it:

```rust
struct Document;

impl Document {
    fn render(&self) {}
}
```

## What it skips

Automatic migrations move generic parameters only when the resolved receiver arguments actually use them; name substrings and other ambiguous syntax receive manual guidance.

## When to turn it off

Turn this lint off when the reported API is deliberate and callers depend on it.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
