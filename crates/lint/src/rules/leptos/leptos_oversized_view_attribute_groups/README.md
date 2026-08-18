# `rlib::leptos_oversized_view_attribute_groups`

## Summary

Finds named Leptos attribute groups whose direct complexity exceeds the configured limit.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos` |
| Cargo feature | `leptos` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds named Leptos attribute groups whose direct complexity exceeds the configured limit.

## Why this matters

A broad heading can legitimize an entire opening-tag interface while leaving the original scanning
problem unchanged. Bounded groups keep comments meaningful and expose extraction pressure.

## Examples

### Triggers the lint

```rust,ignore
<button
    // Button configuration
    id=id type="submit" form=form_id class="primary" disabled=pending on:click=submit
/>
```

### Use this instead

Split stable responsibilities into focused groups, or extract a narrower component:

```rust,ignore
<button
    // Submission identity
    id=id type="submit" form=form_id

    // Availability and behavior
    disabled=pending on:click=submit
/>
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `leptos-view-attribute-group-complexity-threshold` | positive integer | `4` | Sets the largest allowed named attribute group. |

## Known limitations

No known implementation limitations.

## Related lints

- [`rlib::leptos_missing_view_attribute_group_comments`](../leptos_missing_view_attribute_group_comments/README.md) — Finds dense attribute lists with no group names.
- [`rlib::leptos_mismatched_view_attribute_groups`](../leptos_mismatched_view_attribute_groups/README.md) — Checks whether group names match their attributes.
