# Effects synchronizing signals

## Proposition

Add an `effects_synchronizing_signals` lint for Leptos effects that read reactive state and write a
derived value into another signal.

```rust
// Bad: `derived` duplicates data already represented by `source`.
let source = RwSignal::new(1);
let derived = RwSignal::new(2);

Effect::new(move |_| {
    derived.set(source.get() * 2);
});
```

```rust
// Better for cheap computation.
let derived = move || source.get() * 2;

// Better when memoization prevents meaningful downstream work.
let derived = Memo::new(move |_| expensive(source.get()));
```

Leptos effects exist to synchronize the reactive graph with the non-reactive world. Using one to
synchronize two reactive values introduces duplicated state, a second propagation pass, temporary
inconsistency, and opportunities for reactive cycles.

Reference: [Responding to Changes with Effects](https://book.leptos.dev/reactivity/14_create_effect.html).

## Conservative detection

- Recognize `Effect::new`, `Effect::watch`, and configured equivalent constructors.
- Collect tracked signal reads in the effect closure.
- Collect writes through `set`, `update`, `write`, or writable-signal call syntax.
- Diagnose when a written value is computed from one or more tracked reads.
- Increase confidence when the target signal is created immediately before the effect and has no
  independent writers.
- Report the synchronization relationship once rather than reporting every read and write.

## Legitimate effects

```rust
Effect::new(move |_| {
    browser_storage.set_item("theme", theme.get().as_str())?;
});

Effect::new(move |_| {
    logging::log!("selected account: {:?}", selected_account.get());
});
```

Writing to the DOM, browser APIs, storage, logging, telemetry, or another explicitly non-reactive
system is the intended use of an effect. A signal write performed only to record external callback
state may also be legitimate, but it should not derive that state from the same reactive graph.

## Diagnostic direction

The diagnostic should distinguish:

- a cheap derived closure;
- a memo whose equality check avoids expensive downstream work;
- independent signals updated together at the original event boundary;
- a genuine external synchronization effect.

It should not mechanically replace every effect with `Memo`; Leptos memos have their own graph and
comparison overhead.

## Open decisions

- Whether any signal write inside an effect is forbidden or only writes derived from tracked reads.
- Whether writes to stores count as reactive writes for this rule.
- How custom wrappers around `Effect` and signal setters are configured.
- Whether an explicit annotation may designate a deliberate feedback controller.
- Whether writes to a signal returned from a resource belong here or in the resource-specific rule.
