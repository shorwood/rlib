# `rlib::leptos_static_str_component_props`

## Summary

Checks for `&'static str` carried by Leptos component properties, including values nested in generic wrappers and locally defined carrier types.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos` |
| Cargo feature | `leptos` |
| Purpose | Style |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Checks for `&'static str` carried by Leptos component properties, including values nested in
generic wrappers and locally defined carrier types.

## Why this matters

Component properties form the runtime presentation boundary. A static string permanently fixes
text to the program binary, preventing displayed copy from following a user’s active locale and
encouraging technical wire values to remain stringly typed.

## Examples

### Triggers the lint

```rust,ignore
#[component]
fn Notice(label: &'static str, status: &'static str) -> impl IntoView {
    view! { <p class=status>{label}</p> }
}
```

### Use this instead

Use a reactive text property for displayed copy, an owned string for open technical data, and a
domain enum for closed behavior:

```rust,ignore
#[component]
fn Notice(label: TextProp, status: NoticeStatus) -> impl IntoView {
    view! { <p class=status.class_name()>{move || label.get()}</p> }
}
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
