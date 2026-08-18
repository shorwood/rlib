# `rlib::leptos_writable_signal_component_props`

## Summary

Warns about direct and optional component properties proven to implement Leptos's reactive `Write`, `Set`, `Update`, or `UpdateUntracked` capabilities.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos` |
| Cargo feature | `leptos` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Warns about direct and optional component properties proven to implement Leptos's reactive `Write`,
`Set`, `Update`, or `UpdateUntracked` capabilities. A prop used exclusively by one native `bind:*`
is accepted as a transparent control boundary.

## Why this matters

Writable reactive handles let descendants perform arbitrary state transitions owned by an
ancestor. Read-only state plus intent-bearing callbacks keeps mutation ownership explicit,
makes validation and instrumentation discoverable, and narrows the child component's API.

## Examples

### Triggers the lint

```rust
#[component]
fn DeleteButton(selected: RwSignal<Option<UserId>>) -> impl IntoView {
    view! { <button on:click=move |_| selected.set(None)>"Delete"</button> }
}
```

### Use this instead

```rust
#[component]
fn DeleteButton(
    selected: Signal<Option<UserId>>,
    on_delete: Callback<UserId>,
) -> impl IntoView {
    view! { <button on:click=move |_| selected.get().map(|id| on_delete.run(id))>"Delete"</button> }
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
