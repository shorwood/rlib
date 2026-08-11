# Unconsumed generic callables

## Proposition

Extend `unconsumed_generic_abstractions` beyond nominal type declarations to authored generic
functions, inherent impls, methods, and associated declarations whose type parameters never support
a substitution boundary in the active crate.

```rust
fn import<S>(source: S) {
    // ...
}

import(FilesystemSource::new());
```

When `S` is inferred only as `FilesystemSource`, the callable abstraction may add generic
vocabulary and monomorphized surface without serving an observed variant. This proposition remains
separate because call-site inference and method selection require substantially richer evidence
than explicit nominal type paths.

## Conservative detection

- Analyze free functions, inherent impls, methods, and associated declarations independently.
- Resolve inferred substitutions from type-dependent compiler results; reject incomplete or
  ambiguous substitutions rather than guessing from source syntax.
- Require every active call or reference to select the same fully concrete substitution.
- Treat generic forwarding, higher-ranked bounds, opaque types, trait objects, projections,
  dependent generic declarations, function pointers, and escaping callable values as open
  boundaries.
- Count active test calls as consumers, while remaining compilation-local and ignoring
  `cfg`-disabled source.
- Preserve generated declarations, trait-contract parameters, foreign-contract parameters, and
  public publishable-library APIs.
- Diagnose parameters independently and offer no fix when specialization spans multiple
  declarations or changes method selection.

## Open decisions

- Whether a generic function invoked once needs a minimum evidence threshold.
- How turbofish arguments and inferred substitutions should be presented together in diagnostics.
- Whether generic impl parameters consumed by associated items should be diagnosed at the impl or
  at each item.
- How closures, function items, and coercions to function pointers demonstrate a reusable callable
  boundary.
- Whether lifetime and const parameters should join this proposition or remain separate rules.
