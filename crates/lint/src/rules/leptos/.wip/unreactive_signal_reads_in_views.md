# Unreactive signal reads in views

## Proposition

Add an `unreactive_signal_reads_in_views` lint for signal values read once in component setup and
inserted into `view!` as ordinary, permanently frozen values.

```rust
// Bad: the component body runs once and inserts the initial count.
view! {
    <span>{count.get()}</span>
}
```

```rust
// Better: pass the reactive value directly.
view! {
    <span>{count}</span>
}

// Better for a derived value.
view! {
    <span>{move || count.get() * 2}</span>
}
```

Leptos component bodies are setup functions rather than rerendering functions. Signals and closures
are treated as reactive view values; an eagerly computed scalar is not.

Reference: [A Basic Component](https://book.leptos.dev/view/01_basic_component.html).

## Conservative detection

- Inspect authored `view!` invocations inside `#[component]` functions.
- Find signal reads through `get`, `read`, `with`, configured signal-call syntax, or aliases.
- Determine whether the read occurs directly in a child or dynamic attribute expression rather
  than inside a closure accepted as reactive by the view.
- Require that the source signal can change and that the resulting value is not itself reactive.
- Diagnose the outermost frozen expression once.

```rust
let label = count.get().to_string();
view! { <span>{label}</span> }
```

The analysis should follow simple local bindings so moving the eager read one statement above the
macro does not evade the lint.

## Legitimate snapshots

An initial snapshot may be deliberate when the value seeds independent state, records initial
configuration, or is explicitly named accordingly:

```rust
let initial_count = count.get_untracked();
let editor = RwSignal::new(Draft::from(initial_count));
```

This is still architecturally significant and may interact with a future duplicated-prop-state
rule, but it is not necessarily a frozen-view bug.

## Diagnostic direction

- Suggest passing the signal directly for identity presentation.
- Suggest a derived closure for cheap expressions.
- Suggest `Memo` only when the expression is expensive or reused and equality-based suppression is
  meaningful.
- Explain that wrapping the already-frozen local value in a closure does not restore reactivity.

## Open decisions

- How deeply local value flow should be followed before confidence becomes too low.
- Whether `get_untracked` should produce a stronger diagnostic in a view.
- Which component and view macro aliases are configurable.
- Whether static reads inside event handlers or lifecycle callbacks are categorically exempt.
- How view expressions expanded by helper macros retain an authored diagnostic span.
