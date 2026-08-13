# derive_more-aware lints

`derive_more` can replace repetitive trait implementations with a declaration beside the type. The
lints in this layer help teams adopt those derives where the generated behavior is equivalent, and
flag derives that would weaken an invariant or hide an important text format.

There are two kinds of rule:

- adoption rules report hand-written implementations only when `derive_more` can reproduce all of
  their behavior;
- safety rules require evidence that an existing derive bypasses validation or makes a data format
  hard to see.

Some derives overlap with other libraries. The linter never guesses from dependency order. Choose
the provider in the `derive_resolution` section of `dylint.toml`; for example,
`enum_variant_predicates` can select `strum_enum_is` or `derive_more_is_variant`.

## Deliberately absent lint

There is no `derive_more_incoherent_into_iterator_derives` rule for derive_more 2.1. The macro
already requires owned, shared, and mutable iteration to use the same field, so the conflicting
program cannot compile. A lint would add noise without catching a real state.
