# Implicitly defaulted component props

## Proposition

Add an `implicit_default_component_props` lint for Leptos props using `#[prop(optional)]` when
`Default::default()` does not communicate a meaningful component contract.

```rust
// Bad: callers omitting max silently select zero.
#[component]
fn ProgressBar(
    #[prop(optional)] max: u16,
    progress: Signal<u16>,
) -> impl IntoView {
    // ...
}
```

```rust
// Better: the default is visible at the definition.
#[component]
fn ProgressBar(
    #[prop(default = 100)] max: u16,
    progress: Signal<u16>,
) -> impl IntoView {
    // ...
}
```

Leptos supports both implicit optional props and explicit default expressions. Component defaults
are API policy and should be named or visible rather than inherited accidentally from a type's
generic `Default` implementation.

Reference: [Components and Props](https://book.leptos.dev/view/03_components.html).

## Conservative detection

- Inspect parameters on authored `#[component]` functions.
- Diagnose `#[prop(optional)]` on non-`Option<T>` values.
- Increase severity for numbers, booleans, empty strings, empty collections, and domain types whose
  `Default` has no obvious UI meaning.
- Accept `#[prop(default = expression)]` while allowing other rules to inspect the expression.
- Coordinate with `boolean_component_props`, which rejects boolean props regardless of defaulting.

## Absence versus default

`Option<T>` is appropriate when absence is meaningful and the component handles it explicitly:

```rust
#[prop(optional)] subtitle: Option<String>
```

Even here, a proliferation of optional props may indicate an implicit component state machine or an
oversized component interface. This rule should focus on invisible `Default::default()` rather than
claim that every optional prop is good design.

## Diagnostic direction

- Require an explicit default expression when omission means one stable value.
- Recommend `Option<T>` when absence itself affects behavior.
- Recommend a semantic enum when omission selects a mode or presentation state.
- Recommend splitting the component when combinations of optional props form distinct variants.

## Open decisions

- Whether `#[prop(optional)]` on `Option<T>` should always remain valid.
- Whether explicit defaults must use named constants or enum variants rather than literals.
- Whether transparent design-system components may mirror native optional attributes.
- Whether optional callbacks defaulting to no-op should be rejected as hidden capability.
- Whether a separate rule should limit total optional-prop combinations.
