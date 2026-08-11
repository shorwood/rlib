# Hydration-divergent views

## Proposition

Add a `hydration_divergent_views` lint for component setup logic that can produce different initial
view trees during server rendering and browser hydration.

```rust
// Bad: the server and browser construct different node sequences.
let toolbar = if cfg!(target_arch = "wasm32") {
    view! { <ClientToolbar/> }.into_any()
} else {
    view! { <ServerPlaceholder/> }.into_any()
};
```

Hydration walks the browser DOM expecting it to match the server-rendered view. Target-dependent
branches, nondeterministic initial values, and browser-rewritten invalid HTML can violate that
contract.

Reference: [Hydration Bugs](https://book.leptos.dev/ssr/24_hydration_bugs.html).

## Conservative detection

- Find `cfg!(target_arch = "wasm32")`, `cfg!(feature = "ssr")`, or configured runtime environment
  tests controlling `view!` structure.
- Find initial view identity derived from current time, randomness, process-local counters, or
  browser-only storage during component setup.
- Compare branch shapes when both are authored locally and diagnose differing element or text-node
  structure.
- Increase severity when divergent values determine list length, element type, key, or sibling
  order.
- Restrict the lint to crates or components configured for SSR and hydration.

## Safe client-only behavior

Browser-specific work placed in a normal client-only effect does not alter server HTML before
hydration and is often appropriate. A placeholder can be rendered consistently first and updated
reactively after hydration.

```rust
let browser_value = RwSignal::new(None);
Effect::new(move |_| browser_value.set(load_browser_value()));
```

The exact lifecycle primitive may vary; the invariant is that initial server and client trees agree.

## Invalid HTML

Browsers can rewrite invalid element nesting before Leptos hydrates it. That deserves a companion
`invalid_view_element_nesting` lint rather than being folded entirely into environment-divergence
analysis. Both rules protect the same hydration invariant through different evidence.

## Open decisions

- Whether hydration rules are universal or activated only for known SSR builds.
- Which nondeterministic functions are recognized and how wrappers are followed.
- Whether differing text content is as severe as differing node structure.
- How islands and explicitly client-only components declare safe divergence.
- Whether invalid HTML nesting should be developed as the next separate proposition.
