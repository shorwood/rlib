# `rlib::leptos_unreactive_signal_reads_in_views`

## Summary

Checks for tracked signal reads evaluated directly while a Leptos view is first constructed, including fallible clone, guard, and closure-based reads.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos` |
| Cargo feature | `leptos` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Checks for tracked signal reads evaluated directly while a Leptos view is first constructed,
including fallible clone, guard, and closure-based reads.

## Why this matters

A component function runs once to build its view. A value read immediately with `get()` becomes an
ordinary snapshot, so the displayed child or attribute does not update when the signal changes.

## Examples

### Triggers the lint

```rust,ignore
view! {
    <span>{count.get()}</span>
}
```

### Use this instead

Pass the signal directly when displaying its value:

```rust,ignore
view! {
    <span>{count}</span>
}
```

Use a closure when the displayed value is derived:

```rust,ignore
view! {
    <span>{move || count.get() * 2}</span>
}
```

An untracked read can still be appropriate when explicitly taking an initial snapshot for state
that is not expected to update with the source.

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
