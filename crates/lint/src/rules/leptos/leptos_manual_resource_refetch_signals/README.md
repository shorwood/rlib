# leptos_manual_resource_refetch_signals

## What it does

Checks for a signal value that is read and immediately discarded inside a `LocalResource`
fetcher. This pattern is commonly used as a counter or toggle whose only purpose is forcing the
resource to run again.

## Why is this bad?

The discarded value looks like data needed by the request even though it is only an imperative
reload command. It adds a signal and update plumbing around an operation that the resource already
provides directly.

## Example

```rust,ignore
let (revision, set_revision) = signal(0_u32);
let users = LocalResource::new(move || {
    revision.get();
    load_users()
});

set_revision.update(|revision| *revision += 1);
```

## Use instead

Call the resource's explicit refetch operation after the action that changes its data:

```rust,ignore
let users = LocalResource::new(load_users);

users.refetch();
```

Signal reads whose values are used to build the request remain valid reactive dependencies.
