# `rlib::leptos_manual_resource_refetch_signals`

## Summary

Checks for a signal value that is read and immediately discarded inside a `LocalResource` or `ArcLocalResource` fetcher.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos` |
| Cargo feature | `leptos` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Checks for a signal value that is read and immediately discarded inside a `LocalResource` or
`ArcLocalResource` fetcher. This pattern is commonly used as a counter or toggle whose only
purpose is forcing the resource to run again.

## Why this matters

The discarded value looks like data needed by the request even though it is only an imperative
reload command. It adds a signal and update plumbing around an operation that the resource already
provides directly.

## Examples

### Triggers the lint

```rust,ignore
let (revision, set_revision) = signal(0_u32);
let users = LocalResource::new(move || {
    revision.get();
    load_users()
});

set_revision.update(|revision| *revision += 1);
```

### Use this instead

Call the resource's explicit refetch operation after the action that changes its data:

```rust,ignore
let users = LocalResource::new(load_users);

users.refetch();
```

Signal reads whose values are used to build the request remain valid reactive dependencies.

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
