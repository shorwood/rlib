# leptos_reactive_writes_in_resource_fetchers

## What it does

Checks for signal writes performed inside the asynchronous function that loads a Leptos resource.
This covers local, arc, blocking, codec-specific, and options-based resource constructors.

## Why is this bad?

Resource fetchers can run at different times during server rendering, hydration, and client-side
rendering. A resource result may be serialized and reused without running the fetcher again. A
signal write hidden in the fetcher therefore cannot be relied on to happen in every environment.

## Example

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

## Use instead

Return all loaded data as the resource value and derive presentation state from it:

```rust,ignore
let user = Resource::new(move || user_id.get(), load_user);
let selected_team = move || user.get().map(|user| user.default_team);
```

For state changed by a user action, perform the write in that event handler rather than in the
fetcher it triggers.
