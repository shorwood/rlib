# Read-then-replace signal updates

## Proposition

Add a `read_then_replace_signals` lint for state updates that clone or extract a signal's current
value, mutate it locally, and replace the same signal instead of updating it in place.

```rust
// Bad: clone the collection, mutate the clone, replace the collection.
let mut next_users = users.get();
next_users.push(user);
set_users.set(next_users);
```

```rust
// Better: express one bounded mutation.
set_users.update(|users| users.push(user));
```

The in-place form makes the affected reactive node explicit, avoids unnecessary cloning and
allocation, and prevents unrelated code from observing an intermediate local representation.

Reference: [Working with Signals](https://book.leptos.dev/reactivity/working_with_signals.html).

## Conservative detection

- Associate read and write halves returned from `signal`, as well as unified `RwSignal` values.
- Track a local initialized from `get`, `read().clone`, or equivalent access.
- Require one or more mutations of that local followed by `set` on the originating signal.
- Require that the local does not escape, cross an await point, or contribute to another value.
- Diagnose the complete read-mutate-set sequence once.
- Suggest `update` or `try_update` according to whether the mutation's return value is used.

## Scalar updates

```rust
set_count.set(count.get() + 1);
```

This can become:

```rust
set_count.update(|count| *count += 1);
```

For `Copy` scalars, the performance difference is negligible. The remaining arguments are clearer
mutation ownership and avoiding stale read-then-write logic. Whether scalar cases should warn is an
explicit policy choice rather than an automatic consequence of the collection case.

## Legitimate replacement

Replacement is appropriate when the new value is independent of the previous one, when mutation
must be computed outside the reactive borrow, or when transactional validation can fail before any
write occurs. A local clone used for speculative work and committed only after validation may be
intentional.

## Open decisions

- Whether `Copy` scalar updates are included by default.
- Whether transformations expressed as consuming methods should prefer `update`, `try_update`, or
  explicit replacement.
- How much local control flow may occur between read and replacement.
- Whether a failing validation before `set` proves a legitimate transactional copy.
- Whether concurrent asynchronous tasks make stale read-then-set patterns a separate correctness
  diagnostic.
