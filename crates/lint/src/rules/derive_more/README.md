# derive_more-aware lints

This layer distinguishes two policies:

- adoption lints report authored implementations only when derive_more can reproduce the complete
  contract;
- usage lints require semantic evidence that an existing derive weakens a type invariant or hides a
  machine-facing contract.

Overlapping derive providers are selected only through the `derive_resolution` configuration. In
particular, `enum_variant_predicates` chooses between `strum_enum_is` and
`derive_more_is_variant`; dependency order is never a resolution mechanism.

## Deliberately absent lint

`derive_more_incoherent_into_iterator_derives` is not implementable against derive_more 2.1. Its
`IntoIterator` expansion calls `assert_single_enabled_field()` once and generates every requested
owned, shared, and mutable implementation from that same field. The proposed state—different fields
for different receiver forms—is rejected by the macro and cannot reach lint analysis. A lint for it
would therefore be dead policy coupled to an impossible program.
