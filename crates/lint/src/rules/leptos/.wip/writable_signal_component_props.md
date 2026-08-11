# Writable signal component props

## Proposition

Add a `writable_signal_component_props` lint for component APIs that pass unrestricted reactive
mutation capability into child components instead of domain events or narrow callbacks.

```rust
// Bad: the child can perform any mutation at any time.
#[component]
fn DeleteButton(selected: RwSignal<Option<UserId>>) -> impl IntoView {
    // ...
}
```

```rust
// Better: observation and intent are separate capabilities.
#[component]
fn DeleteButton(
    selected: Signal<Option<UserId>>,
    on_delete: Callback<UserId>,
) -> impl IntoView {
    // ...
}
```

Leptos supports passing writable signals but warns that doing so permits mutation from arbitrary
parts of the component tree.

Reference: [Parent-Child Communication](https://book.leptos.dev/view/08_parent_child.html).

## Conservative detection

- Inspect props of authored `#[component]` functions.
- Diagnose `WriteSignal<T>`, `RwSignal<T>`, writable store fields, and generic types satisfying known
  write traits.
- Increase confidence when the component performs more than one kind of mutation.
- Recommend a read-only reactive value plus one or more intent-bearing callbacks or event types.
- Avoid automatic fixes because ownership and command vocabulary require domain decisions.

## Transparent control primitives

Low-level form controls and design-system adapters may intentionally expose two-way binding:

```rust
#[component]
fn TextInput(value: RwSignal<String>) -> impl IntoView {
    view! { <input bind:value=value/> }
}
```

Possible policies include configured transparent components, an explicit marker attribute, or
allowing writable props only when the component forwards the capability directly to one native
binding without other business logic.

## Why callbacks can be better

Callbacks name allowed state transitions, preserve ownership in the parent, permit validation and
logging at the boundary, and prevent a child from discovering unrelated mutations. Typed events are
preferable to callbacks carrying ambiguous booleans or primitive tuples.

## Open decisions

- Whether writable props are forbidden universally or exempted for transparent controls.
- Whether `Callback<T>` is sufficient or domain event enums are required.
- Whether a child receiving only `WriteSignal<T>` is more or less acceptable than `RwSignal<T>`.
- Whether contexts carrying writable signals receive the same rule.
- How component event-listener syntax affects the recommended replacement.
