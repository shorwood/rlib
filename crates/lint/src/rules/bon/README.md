# Bon lints

## Summary

Rules for Bon builders, including construction safety, public API shape, compatibility, and generated member behavior.

## How to enable

Enable the `bon` Cargo feature to register this family. Enable the whole family with `rlib::bon`.

## Lints

| Lint | Summary | Purpose | Fix |
| --- | --- | --- | --- |
| [`rlib::bon_builders_bypassing_construction_invariants`](./bon_builders_bypassing_construction_invariants/README.md) | Finds Bon-derived structs with fields less visible than the struct itself and an inherent fallible constructor that builds the same type. | Safety | Manual |
| [`rlib::bon_escaping_incomplete_builders`](./bon_escaping_incomplete_builders/README.md) | Finds generated Bon builders returned from private functions or stored in hand-written fields instead of being completed within a focused construction expression. | Safety | Manual |
| [`rlib::bon_implicit_optional_builder_members`](./bon_implicit_optional_builder_members/README.md) | Finds policy-bearing `Option<T>` arguments on Bon-generated free-function and inherent-method builders that Bon makes omittable without an explicit policy. | API design | Manual |
| [`rlib::bon_incoherent_builder_vocabulary`](./bon_incoherent_builder_vocabulary/README.md) | Finds explicit Bon start, finish, or member names that replace established domain terms with generic names on free-function builders, associated-function builders, and derived struct builders. | Code clarity | Manual |
| [`rlib::bon_incoherent_conditional_builder_members`](./bon_incoherent_conditional_builder_members/README.md) | Finds Bon members on derived structs, free functions, and associated functions whose requiredness, default, hidden initialization, or positional placement is changed with `cfg_attr` while the member itself remains present. | Code clarity | Manual |
| [`rlib::bon_inconsistent_builder_conversions`](./bon_inconsistent_builder_conversions/README.md) | Finds same-typed members of one Bon free-function, associated-function, or derived-struct builder that arbitrarily mix `#[builder(into)]` with strict input types. | API design | Manual |
| [`rlib::bon_manual_builder_implementations`](./bon_manual_builder_implementations/README.md) | Finds dedicated `*Builder` structs with a start method, distinct field-named consuming setters covering their state, and an infallible terminal method producing a named type that Bon can generate. | API design | Manual |
| [`rlib::bon_needless_builders_for_small_apis`](./bon_needless_builders_for_small_apis/README.md) | Finds uncustomized private Bon free-function and associated-function builders with only one or two required parameters. | API design | Manual |
| [`rlib::bon_parameter_heavy_apis_without_builders`](./bon_parameter_heavy_apis_without_builders/README.md) | Finds exported free functions and inherent methods with enough positional parameters and ambiguity that a Bon named builder would materially improve calls. | API design | Manual |
| [`rlib::bon_public_builder_implementation_types`](./bon_public_builder_implementation_types/README.md) | Finds generated Bon builder or generated builder state definitions named in hand-written exported function, inherent-method, or field types, including through nested containers and pointer types. | API design | Manual |
| [`rlib::bon_redundant_positional_and_builder_apis`](./bon_redundant_positional_and_builder_apis/README.md) | Finds complex externally reachable Bon function builders that keep most inputs positional through exact member-level `start_fn` or `finish_fn` configuration. | API design | Manual |
| [`rlib::bon_required_builder_members_breaking_compatibility`](./bon_required_builder_members_breaking_compatibility/README.md) | Compares public Bon struct, free-function, and associated-function builders with the explicitly configured `bon_api_baseline` member snapshot and finds newly added required members. | API design | Manual |
| [`rlib::bon_skipped_builder_members_without_policy`](./bon_skipped_builder_members_without_policy/README.md) | Finds fields with bare `#[builder(skip)]` policy and no substantive documentation on Bon-derived structs. | API design | Manual |
| [`rlib::bon_undocumented_builder_members`](./bon_undocumented_builder_members/README.md) | Finds non-obvious optionality, default, conversion, or hidden-initialization behavior on undocumented members of externally reachable Bon struct, free-function, and associated-function builders. | Code clarity | Manual |
