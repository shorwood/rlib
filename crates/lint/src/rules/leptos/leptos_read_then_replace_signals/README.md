# `rlib::leptos_read_then_replace_signals`

## Summary

Checks for a writable signal that reads its current value to calculate the value passed back to `set()` or `try_set()`.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos` |
| Cargo feature | `leptos` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Checks for a writable signal that reads its current value to calculate the value passed back to
`set()` or `try_set()`. Tracked, untracked, fallible, borrowed, and closure-based read operations
all represent the same read-then-replace pattern.

## Why this matters

Reading and then replacing reactive state separates one logical change into two operations. The
read may clone the stored value, and asynchronous or re-entrant code can make the replacement use
an older snapshot.

## Examples

### Triggers the lint

```rust,ignore
items.set({
    let mut next = items.get();
    next.push(item);
    next
});
```

### Use this instead

Keep the change inside one bounded update:

```rust,ignore
items.update(|items| items.push(item));
```

The same applies to small scalar changes: `count.update(|count| *count += 1)` keeps the read and
write together. Direct replacement remains appropriate when the new value does not depend on the
signal's current value.

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

- [`rlib::leptos_effects_synchronizing_signals`](../leptos_effects_synchronizing_signals/README.md) — Covers effects that copy one reactive value into another.
