# Primitive context values

## Proposition

Add a `primitive_context_values` lint for values passed through Leptos context without a domain
wrapper that uniquely identifies their role.

```rust
// Bad: contexts are identified by type, so both values have the same identity.
provide_context(set_theme);
provide_context(set_sidebar);

let setter = use_context::<WriteSignal<bool>>();
```

```rust
// Better: the context type names its capability.
#[derive(Clone, Copy)]
struct ThemeContext(WriteSignal<Theme>);

#[derive(Clone, Copy)]
struct SidebarContext(WriteSignal<SidebarState>);
```

Leptos context lookup is type-based and runtime-optional. The official guide recommends newtypes to
avoid collisions and make context dependencies easier to name and refactor.

Reference: [Parent-Child Communication](https://book.leptos.dev/view/08_parent_child.html#41-the-context-api).

## Conservative detection

- Inspect arguments to `provide_context` and type arguments to `use_context` or `expect_context`.
- Diagnose primitives, `String`, generic collections, raw callbacks, and unbranded reactive handles.
- Diagnose common wrappers such as `ReadSignal<T>`, `WriteSignal<T>`, and `RwSignal<T>` when the outer
  type alone does not identify the domain role.
- Pair providers and consumers within the crate when possible.
- Recommend a local named wrapper whose field type preserves the original capability.

## Capability design

A context wrapper should expose the narrowest authority required by descendants:

```rust
struct ThemeContext {
    current: Signal<Theme>,
    select: Callback<Theme>,
}
```

Passing an `RwSignal` may grant read and write access where a read-only signal plus intent callback
would be clearer. This lint should identify ambiguous context identity; a separate writable-signal
rule can address excessive authority.

## Legitimate exceptions

Framework-defined unique context types are already branded. Application state structs, service
handles, and dedicated capability types should pass without additional wrapping. Generated external
types may be configurable when their type identity is stable and semantically unique.

## Open decisions

- Which outer generic types are intrinsically too broad for context.
- Whether a local named type alias is sufficient or a nominal newtype is required.
- Whether `expect_context` should itself be discouraged in reusable components.
- How shadowing the same context type in nested owners should be treated.
- Whether provider-consumer mismatches deserve a separate architecture diagnostic.
