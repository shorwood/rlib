# `rlib::leptos_mismatched_view_attribute_groups`

## Summary

Finds Leptos attributes placed beneath a group heading that explicitly names a contradictory behavioral category.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos` |
| Cargo feature | `leptos` |
| Purpose | Correctness |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds Leptos attributes placed beneath a group heading that explicitly names a contradictory
behavioral category.

## Why this matters

An attribute-group heading tells readers what the following attributes do. A behavior binding beneath a presentation
heading, for example, turns that claim into misleading decoration.

## Examples

### Triggers the lint

```rust,ignore
<button
    // Submission presentation
    class="primary"
    on:click=submit
/>
```

### Use this instead

Move the binding beneath a matching responsibility or rename a genuinely cross-cutting group:

```rust,ignore
<button
    // Submission presentation
    class="primary"

    // Submission behavior
    on:click=submit
/>
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

- [`rlib::leptos_missing_view_attribute_group_comments`](../leptos_missing_view_attribute_group_comments/README.md) — Finds dense attribute lists with no group names.
- [`rlib::leptos_oversized_view_attribute_groups`](../leptos_oversized_view_attribute_groups/README.md) — Limits the size of existing attribute groups.
