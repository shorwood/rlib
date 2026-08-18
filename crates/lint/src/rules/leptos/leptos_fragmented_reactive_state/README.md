# `rlib::leptos_fragmented_reactive_state`

## Summary

Limits the reactive primitives created directly by each component and composable.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos` |
| Cargo feature | `leptos` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Limits the reactive primitives created directly by each component and composable.

## Why this matters

Many independent signals hide cohesive state transitions and permit invalid intermediate combinations.

## Examples

### Triggers the lint

```rust,ignore
let name = RwSignal::new(String::new());
let email = RwSignal::new(String::new());
let phone = RwSignal::new(String::new());
let city = RwSignal::new(String::new());
let country = RwSignal::new(String::new());
```

### Use this instead

```rust,ignore
let draft = RwSignal::new(ContactDraft::default());
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `leptos-reactive-primitives-threshold` | positive integer | `4` | Sets how many separate reactive values may be created in one component. |

## Known limitations

No known implementation limitations.

## Related lints

None.
