# leptos_resource_fetchers_rereading_sources

## What it does

Finds `Resource` fetchers that reread a reactive value already tracked by their source closure,
whether or not they also use the supplied source argument. This covers the standard, arc,
blocking, codec-specific, and options-based constructors.

## Why is this bad?

Leptos tracks reactive reads in the source closure, but fetcher reads are untracked. Rereading a
captured signal can therefore observe a different value from the source value that triggered the
load and obscures the resource's actual input contract.

## Example

```rust,ignore
let users = Resource::new(
    move || organization_id.get(),
    |_| load_users(organization_id.get()),
);
```

## Use instead

Treat the source closure's result as the fetcher's complete input:

```rust,ignore
let users = Resource::new(
    move || organization_id.get(),
    |organization_id| load_users(organization_id),
);
```
