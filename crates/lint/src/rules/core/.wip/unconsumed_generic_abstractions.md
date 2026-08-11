# Unconsumed generic abstractions

## Proposition

Diagnose authored generic declarations whose type parameters never support a substitution boundary
in the active crate. The rule should distinguish a reusable algorithm or data structure from a
generic parameter introduced only to make a concrete design appear extensible.

```rust
struct Importer<S> {
    source: S,
}

impl Importer<FilesystemSource> {
    fn import(&self) {}
}
```

When `S` is instantiated only as `FilesystemSource`, the generic abstraction may add monomorphized
surface and indirect vocabulary without serving an observed variant.

## Conservative detection

- Analyze functions, inherent impls, structs, enums, unions, aliases, and associated declarations
  independently; do not infer one proposition from syntax alone.
- Require every active use to resolve to the same concrete substitution and reject partial,
  inferred, or unresolved evidence.
- Treat generic forwarding, higher-ranked bounds, opaque types, trait objects, projections,
  dependent generic declarations, and public publishable-library APIs as real open boundaries.
- Count active test substitutions as consumers, while remaining compilation-local and ignoring
  `cfg`-disabled source.
- Exempt generated declarations and parameters required by a foreign or language-level contract.
- Explain the observed substitution and recommend replacing the parameter with the concrete type;
  offer no fix where removing the parameter changes multiple declarations.

## Open decisions

- Whether a parameter used by several concrete types that normalize to one representation is
  consumed meaningfully.
- How const and lifetime parameters should participate in the same proposition.
- Whether generic functions invoked once deserve a minimum use threshold before diagnosis.
- How to represent substitutions across mutually recursive aliases without unstable inference.

