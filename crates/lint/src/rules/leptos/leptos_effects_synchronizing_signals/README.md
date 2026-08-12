# leptos_effects_synchronizing_signals

## What it does

Checks for Leptos effects that read tracked reactive state and write reactive state in the same
effect callback.

## Why is this bad?

An effect runs after its dependencies change. Copying a derived value into another signal creates
two sources of truth, performs an extra reactive update, and can briefly expose inconsistent state.
It can also create a reactive cycle.

Effects are best used to synchronize reactive state with systems outside the reactive graph, such
as the browser, logging, or storage.

## Example

```rust,ignore
let count = RwSignal::new(1);
let doubled = RwSignal::new(2);

Effect::new(move |_| {
    doubled.set(count.get() * 2);
});
```

## Use instead

Derive inexpensive values directly:

```rust,ignore
let doubled = move || count.get() * 2;
```

Use a memo when equality checking avoids meaningful downstream work:

```rust,ignore
let doubled = Memo::new(move |_| expensive_double(count.get()));
```
