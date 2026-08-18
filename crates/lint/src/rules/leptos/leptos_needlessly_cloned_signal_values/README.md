# `rlib::leptos_needlessly_cloned_signal_values`

## Summary

Checks for a non-copy signal value read with `get()` and immediately inspected with `len()` or `is_empty()`.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos` |
| Cargo feature | `leptos` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Checks for a non-copy signal value read with `get()` and immediately inspected with `len()` or
`is_empty()`. Reads whose values are moved, stored, or independently modified are accepted.

## Why this matters

Signal `get()` clones the complete stored value. Cloning a string or collection merely to inspect
its length creates unnecessary allocation and copying on every reactive run.

## Examples

### Triggers the lint

```rust,ignore
let users = RwSignal::new(Vec::<User>::new());

if users.get().is_empty() {
    // ...
}
```

### Use this instead

Borrow the value for the short inspection:

```rust,ignore
if users.read().is_empty() {
    // ...
}
```

For a longer calculation, `with()` clearly limits how long the borrow is retained:

```rust,ignore
let has_admin = users.with(|users| users.iter().any(User::is_admin));
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
