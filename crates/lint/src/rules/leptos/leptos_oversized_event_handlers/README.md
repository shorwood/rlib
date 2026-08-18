# `rlib::leptos_oversized_event_handlers`

## Summary

Limits statement count and nested control flow in `on:*` handlers and `Callback::new` closures.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos` |
| Cargo feature | `leptos` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Limits statement count and nested control flow in `on:*` handlers and `Callback::new` closures.

## Why this matters

Complex inline handlers conceal intent and leave application behavior difficult to test independently.

## Examples

### Triggers the lint

```rust,ignore
view! { <button on:click=move |_| { validate(); save(); refresh(); close(); }>"Save"</button> }
```

### Use this instead

```rust,ignore
let save = use_save_contact();
view! { <button on:click=move |_| save.dispatch(())>"Save"</button> }
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `leptos-event-handler-statements-threshold` | positive integer | `3` | Sets how many statements may appear in one event handler. |
| `leptos-event-handler-control-flow-depth-threshold` | positive integer | `1` | Sets the deepest allowed control flow inside an event handler. |

## Known limitations

No known implementation limitations.

## Related lints

None.
