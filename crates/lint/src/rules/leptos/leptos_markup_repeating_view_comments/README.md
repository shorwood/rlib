# `rlib::leptos_markup_repeating_view_comments`

## Summary

Checks for one-node view sections whose heading only repeats the node’s name or visible label.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos` |
| Cargo feature | `leptos` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Checks for one-node view sections whose heading only repeats the node’s name or visible label.

## Why this matters

Comments are useful when they add meaning that the code cannot already express. Repeating the
markup makes the view longer without helping readers understand why the region exists.

## Examples

### Triggers the lint

```rust,ignore
view! {
    // Navigation
    <Navigation/>

    // Submit button
    <button>"Submit"</button>
}
```

### Use this instead

Remove the comment when the markup is already clear, or describe the region’s responsibility:

```rust,ignore
view! {
    // Account shortcuts
    <Navigation/>

    <button>"Submit"</button>
}
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
