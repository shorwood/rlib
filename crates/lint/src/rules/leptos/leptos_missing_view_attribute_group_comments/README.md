# `rlib::leptos_missing_view_attribute_group_comments`

## Summary

Finds dense Leptos opening tags whose attributes span several responsibilities without named groups.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos` |
| Cargo feature | `leptos` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds dense Leptos opening tags whose attributes span several responsibilities without named
groups.

## Why this matters

A large opening tag is a compact interface definition. When identity, state, accessibility,
presentation, and behavior are interleaved, readers must repeatedly classify every binding before
they can understand what the element does.

## Examples

### Triggers the lint

```rust,ignore
<button type="submit" form=form_id class="primary" class:pending=pending
    disabled=pending aria-busy=pending on:click=submit>
    "Save"
</button>
```

### Use this instead

Name stable, element-specific responsibilities, or extract a narrower component:

```rust,ignore
<button
    // Submission identity
    type="submit" form=form_id

    // Availability and progress
    disabled=pending aria-busy=pending

    // Submission presentation and behavior
    class="primary" class:pending=pending on:click=submit
>
    "Save"
</button>
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `leptos-unnamed-view-attribute-complexity-threshold` | positive integer | `6` | Sets when an element's attributes need short group headings. |

## Known limitations

No known implementation limitations.

## Related lints

- [`rlib::leptos_mismatched_view_attribute_groups`](../leptos_mismatched_view_attribute_groups/README.md) — Checks whether existing group names match their attributes.
- [`rlib::leptos_oversized_view_attribute_groups`](../leptos_oversized_view_attribute_groups/README.md) — Limits the size of existing attribute groups.
