# Needless delegating types

## Proposition

Diagnose authored wrapper types whose API merely forwards to one stored value without establishing
a distinct invariant, representation, ownership boundary, or behavioral policy.

```rust
struct UserStore(DatabaseUserStore);

impl UserStore {
    fn find(&self, id: UserId) -> Result<User> {
        self.0.find(id)
    }
}
```

When every operation delegates unchanged, the wrapper creates another name and navigation layer but
does not yet own a meaningful contract.

## Conservative detection

- Begin with single-field tuple and record structs whose authored methods forward receiver,
  arguments, return value, and error behavior directly to that field.
- Require all construction and destruction sites to preserve the wrapped value unchanged.
- Treat validation, normalization, access control, caching, synchronization, conversion, error
  translation, lifecycle behavior, and representation hiding as substantive ownership.
- Preserve newtypes that implement foreign traits, participate in coherence, control auto traits,
  alter variance or drop behavior, provide FFI representation, or encode units and domain identity.
- Preserve exported publishable-library types because downstream invariants and compatibility
  consumers are not observable.
- Count active test behavior and remain compilation-local; ignore generated declarations and
  `cfg`-disabled source.
- Label the wrapper, stored field, and representative forwarding methods, then recommend using the
  wrapped type directly or moving a real invariant into the wrapper. Offer no automatic rewrite.

## Open decisions

- What minimum proportion of forwarding methods proves the whole type is delegating.
- Whether a private constructor alone is sufficient evidence of an invariant.
- How to classify wrappers used primarily for dependency injection or framework extraction.
- Whether serialization names and stable wire shapes constitute representation ownership.
