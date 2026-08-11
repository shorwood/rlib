# Needlessly cloned signal values

## Proposition

Add a `needlessly_cloned_signal_values` lint for `.get()` calls that clone an entire reactive value
only to perform a borrowing operation.

```rust
// Bad: clones every User to ask whether the collection is empty.
if users.get().is_empty() {
    // ...
}
```

```rust
// Better for a short direct borrow.
if users.read().is_empty() {
    // ...
}

// Better for bounded multi-step borrowing.
let has_admin = users.with(|users| users.iter().any(User::is_admin));
```

Leptos documents that `.get()` clones the stored value, while `.read()` and `.with()` provide
tracked borrowing access.

Reference: [Working with Signals](https://book.leptos.dev/reactivity/working_with_signals.html).

## Conservative detection

- Resolve `.get()` to a Leptos reactive read trait rather than matching method spelling alone.
- Require a non-`Copy` stored type.
- Diagnose immediate borrowing consumers such as `len`, `is_empty`, `contains`, indexing,
  comparison by reference, iterator inspection, or a callee accepting only `&T`.
- Follow short method chains while proving that ownership never escapes.
- Recommend `.read()` for a short expression and `.with()` where a guard would otherwise live too
  long.

```rust
users.get().iter().any(User::is_admin)
document.get().title()
settings.get() == expected_settings
```

## Borrow guard hazards

A read guard prevents conflicting writes until it is dropped and can produce runtime errors if held
across a write. The diagnostic must avoid replacing `.get()` with `.read()` when the borrow would
cross:

- a write to the same reactive value;
- an `.await` point;
- a callback with unknown behavior;
- storage in a longer-lived value.

`with` is often the safer suggestion because it bounds the borrow explicitly.

## Legitimate cloning

`.get()` is correct when the caller moves, stores, returns, asynchronously owns, or independently
mutates the resulting value. It is also reasonable for small `Copy`-like values even though the API
spells the operation as a clone.

## Open decisions

- Whether inexpensive `Clone` types such as `String` should still be diagnosed for consistency.
- Which known borrowing methods deserve built-in recognition.
- Whether a cost threshold based on type structure is preferable to a simple `Copy` distinction.
- Whether nightly signal-call syntax should receive the same analysis.
- Whether resource `.get()` returning cloned `Option<T>` belongs to this rule.
