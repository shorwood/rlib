# `rlib::leptos_repeated_view_fragments`

## Summary

Finds crate-wide normalized RSX subtrees repeated often enough to represent a missing component.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos` |
| Cargo feature | `leptos` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds crate-wide normalized RSX subtrees repeated often enough to represent a missing component.

## Why this matters

Copied markup drifts independently and leaves a shared visual concept implicit.

## Examples

### Triggers the lint

```rust,ignore
view! { <label class=style::FIELD><span>"Name"</span><input prop:value=name /></label> }
view! { <label class=style::FIELD><span>"Email"</span><input prop:value=email /></label> }
```

### Use this instead

```rust,ignore
view! { <FormField label="Name" value=name /><FormField label="Email" value=email /> }
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `leptos-repeated-view-fragment-nodes-threshold` | positive integer | `6` | Sets the smallest repeated fragment, measured in view nodes. |
| `leptos-repeated-view-fragment-occurrences-threshold` | positive integer | `2` | Sets how many matching fragments count as repetition. |

## Known limitations

No known implementation limitations.

## Related lints

None.
