# `rlib::leptos_excessively_nested_views`

## Summary

Limits static tag nesting and embedded Rust control-flow nesting within a hand-written component view.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos` |
| Cargo feature | `leptos` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Limits static tag nesting and embedded Rust control-flow nesting within a hand-written component view.

## Why this matters

Deep views obscure visual hierarchy, component states, and accessible structure.

## Examples

### Triggers the lint

```rust,ignore
view! { <main><section><div><div><div><div><div><div>"value"</div></div></div></div></div></div></section></main> }
```

### Use this instead

```rust,ignore
view! { <main><SummarySection /></main> }
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `leptos-view-nesting-depth-threshold` | positive integer | `7` | Sets the deepest allowed element nesting in a view. |
| `leptos-view-control-flow-depth-threshold` | positive integer | `3` | Sets the deepest allowed control-flow nesting inside a view. |

## Known limitations

No known implementation limitations.

## Related lints

None.
