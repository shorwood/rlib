# leptos_unreactive_signal_reads_in_views

## What it does

Checks for tracked signal reads evaluated directly while a Leptos view is first constructed.

## Why is this bad?

A component function runs once to build its view. A value read immediately with `get()` becomes an
ordinary snapshot, so the displayed child or attribute does not update when the signal changes.

## Example

```rust,ignore
view! {
    <span>{count.get()}</span>
}
```

## Use instead

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
