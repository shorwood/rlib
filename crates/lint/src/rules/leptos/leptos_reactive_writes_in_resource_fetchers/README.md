# `rlib::leptos_reactive_writes_in_resource_fetchers`

## Summary

Checks for signal writes performed inside the asynchronous function that loads a Leptos resource.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos` |
| Cargo feature | `leptos` |
| Purpose | Safety |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Checks for signal writes performed inside the asynchronous function that loads a Leptos resource.
This covers local, arc, blocking, codec-specific, and options-based resource constructors.

## Why this matters

Resource fetchers can run at different times during server rendering, hydration, and client-side
rendering. A resource result may be serialized and reused without running the fetcher again. A
signal write hidden in the fetcher therefore cannot be relied on to happen in every environment.

## Examples

### Triggers the lint

```rust,ignore
let selected_team = RwSignal::new(None);
let user = Resource::new(
    move || user_id.get(),
    move |user_id| async move {
        let user = load_user(user_id).await?;
        selected_team.set(Some(user.default_team));
        Ok(user)
    },
);
```

### Use this instead

Return all loaded data as the resource value and derive presentation state from it:

```rust,ignore
let user = Resource::new(move || user_id.get(), load_user);
let selected_team = move || user.get().map(|user| user.default_team);
```

For state changed by a user action, perform the write in that event handler rather than in the
fetcher it triggers.

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off only when the risk is handled elsewhere and documented.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
