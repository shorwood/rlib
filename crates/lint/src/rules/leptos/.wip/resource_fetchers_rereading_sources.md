# Resource fetchers rereading sources

## Proposition

Add a `resource_fetchers_rereading_sources` lint for `Resource` fetchers that ignore their source
argument and recapture the same reactive inputs inside the fetcher.

```rust
// Bad: organization_id is declared as source input, discarded, and reread.
let users = Resource::new(
    move || organization_id.get(),
    |_| load_users(organization_id.get()),
);
```

```rust
// Better: the source value is the fetcher's complete input.
let users = Resource::new(
    move || organization_id.get(),
    |organization_id| load_users(organization_id),
);
```

Leptos tracks signals read by the resource source function. Reads inside the fetcher are untracked,
which is essential to resource reloading and hydration behavior.

Reference: [Loading Data with Resources](https://book.leptos.dev/async/10_resources.html).

## Conservative detection

- Resolve calls to `Resource::new` and configured wrappers with separate source and fetcher
  closures.
- Record reactive dependencies read in the source closure.
- Detect ignored or partially destructured source parameters.
- Detect the same reactive values captured and reread in the fetcher.
- Increase confidence when the reread value is passed to the primary async operation.
- Diagnose the resource construction once and identify the duplicated dependency.

## Why this is dangerous

The code visually declares one dependency graph while executing another. A fetcher reread does not
become a tracked resource dependency, can observe a value different from the memoized source, and
obscures which value was serialized or reused during hydration.

## Legitimate ignored sources

A source may intentionally act as a refresh trigger:

```rust
Resource::new(move || refresh_token.get(), |_| load_current_user())
```

This is legitimate when the fetcher truly needs no source data. The lint should only diagnose a
captured reread that duplicates or bypasses the source. A named trigger type or explicit `refetch`
operation may still communicate the design more clearly.

## Open decisions

- Whether every ignored non-unit source parameter is suspicious even without a reread.
- How tuple and struct source values are compared with fetcher captures.
- Whether `LocalResource`, whose tracking model differs, belongs to a separate analysis.
- How custom resource constructors declare which closure is the source and which is the fetcher.
- Whether stale-value risk warrants a correctness-level diagnostic rather than a style lint.
