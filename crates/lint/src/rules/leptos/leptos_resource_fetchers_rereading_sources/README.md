# `rlib::leptos_resource_fetchers_rereading_sources`

## Summary

Finds `Resource` fetchers that reread a reactive value already tracked by their source closure, whether or not they also use the supplied source argument.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos` |
| Cargo feature | `leptos` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds `Resource` fetchers that reread a reactive value already tracked by their source closure,
whether or not they also use the supplied source argument. This covers the standard, arc,
blocking, codec-specific, and options-based constructors.

## Why this matters

Leptos tracks reactive reads in the source closure, but fetcher reads are untracked. Rereading a
captured signal can therefore observe a different value from the source value that triggered the
load and obscures the value that actually controls the resource.

## Examples

### Triggers the lint

```rust,ignore
let users = Resource::new(
    move || organization_id.get(),
    |_| load_users(organization_id.get()),
);
```

### Use this instead

Treat the source closure's result as the fetcher's complete input:

```rust,ignore
let users = Resource::new(
    move || organization_id.get(),
    |organization_id| load_users(organization_id),
);
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
