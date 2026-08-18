# `rlib::leptos_effects_synchronizing_signals`

## Summary

Checks for Leptos effects that read tracked reactive state and write reactive state in the same effect callback.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos` |
| Cargo feature | `leptos` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Checks for Leptos effects that read tracked reactive state and write reactive state in the same
effect callback.

## Why this matters

An effect runs after its dependencies change. Copying a derived value into another signal creates
two sources of truth, performs an extra reactive update, and can briefly expose inconsistent state.
It can also create a reactive cycle.

Effects are best used to synchronize reactive state with systems outside the reactive graph, such
as the browser, logging, or storage.

## Examples

### Triggers the lint

```rust,ignore
let count = RwSignal::new(1);
let doubled = RwSignal::new(2);

Effect::new(move |_| {
    doubled.set(count.get() * 2);
});
```

### Use this instead

Derive inexpensive values directly:

```rust,ignore
let doubled = move || count.get() * 2;
```

Use a memo when equality checking avoids meaningful downstream work:

```rust,ignore
let doubled = Memo::new(move |_| expensive_double(count.get()));
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

- [`rlib::leptos_read_then_replace_signals`](../leptos_read_then_replace_signals/README.md) — Finds a related read-then-write signal pattern.
- [`rlib::leptos_reactive_writes_during_view_construction`](../leptos_reactive_writes_during_view_construction/README.md) — Covers writes performed while building a view.
