# leptos_needlessly_cloned_signal_values

## What it does

Checks for a non-copy signal value read with `get()` and immediately inspected with `len()` or
`is_empty()`. Reads whose values are moved, stored, or independently modified are accepted.

## Why is this bad?

Signal `get()` clones the complete stored value. Cloning a string or collection merely to inspect
its length creates unnecessary allocation and copying on every reactive run.

## Example

```rust,ignore
let users = RwSignal::new(Vec::<User>::new());

if users.get().is_empty() {
    // ...
}
```

## Use instead

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
