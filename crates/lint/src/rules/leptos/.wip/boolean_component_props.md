# Boolean component props

## Proposition

Add a `boolean_component_props` lint that rejects boolean parameters on functions annotated with
Leptos's `#[component]` macro.

```rust
// Bad: the value at the call site does not name the state it selects.
#[component]
fn AccountBadge(is_active: bool) -> impl IntoView {
    // ...
}

view! { <AccountBadge is_active=true/> }
```

```rust
// Better: the component API exposes a domain state.
enum AccountStatus {
    Active,
    Inactive,
}

#[component]
fn AccountBadge(status: AccountStatus) -> impl IntoView {
    // ...
}

view! { <AccountBadge status=AccountStatus::Active/> }
```

Component props form a declarative UI vocabulary. A boolean compresses that vocabulary into a flag
whose opposite state, future states, and interactions with other flags remain implicit. An enum
makes every state name visible at both the component definition and each `view!` call site.

## Why Leptos needs the stricter rule

The core `boolean_function_arguments` proposal may allow obvious setters or predicate-shaped
parameters. Component props deserve stricter treatment because component calls resemble markup:
the prop value should explain the selected presentation or behavior without requiring readers to
open the component implementation.

Multiple booleans also permit invalid or contradictory combinations:

```rust
// Bad: what does active=true, suspended=true mean?
#[component]
fn AccountBadge(active: bool, suspended: bool, compact: bool) -> impl IntoView {
    // ...
}
```

```rust
// Better: domain state and presentation density are independent named dimensions.
enum AccountStatus {
    Active,
    Suspended,
}

enum BadgeDensity {
    Comfortable,
    Compact,
}
```

## Conservative detection

- Inspect authored functions carrying Leptos's `#[component]` attribute.
- Diagnose every explicit `bool` prop, including wrapped reactive forms such as `Signal<bool>`,
  `ReadSignal<bool>`, `RwSignal<bool>`, and `MaybeSignal<bool>` when their value selects component
  state.
- Diagnose optional boolean props and boolean props with a default value.
- Point at the prop declaration and show the corresponding component-call ambiguity when a local
  call site is available.
- Recommend a domain enum derived from the component and prop names rather than mechanically
  suggesting `Enabled` and `Disabled`.
- Avoid machine-applicable fixes because variant names and state modeling require semantic judgment.

## Event and predicate exemptions

The rule concerns values passed into a component, not every boolean appearing in its signature or
implementation. It should not diagnose:

- callback return values used as predicates;
- DOM event data;
- internal derived predicates;
- values produced by the component rather than configured by its caller.

```rust
#[component]
fn FilteredList(predicate: Callback<Item, bool>) -> impl IntoView {
    // The bool is a callback result, not a component state prop.
}
```

A callback accepting a boolean may still deserve a domain event type under a separate rule:

```rust
// Suspicious: true and false have unnamed event meanings.
on_open_change: Callback<bool>
```

## Interaction with HTML attributes

Native HTML boolean attributes such as `disabled`, `checked`, `required`, and `hidden` are defined by
the platform and remain valid on lowercase elements:

```rust
view! { <button disabled=move || !can_submit.get()>"Submit"</button> }
```

The lint must distinguish Leptos component props from native element attributes. A component that
simply forwards a boolean HTML attribute is still a component API and should normally expose a
domain state or a purpose-specific variant rather than leaking raw platform configuration.

## Open decisions

- Whether reactive boolean props are always forbidden or may be accepted for transparent primitive
  wrappers.
- Whether `Callback<bool>` inputs belong to this lint or a broader boolean-event rule.
- Whether a two-variant enum must use semantic variants rather than generic `Enabled` and
  `Disabled`.
- Whether generated component props outside authored macro invocations should be ignored.
- Whether a project may designate a small set of transparent design-system components that mirror
  native HTML boolean attributes.
