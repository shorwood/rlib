# Derive More lints

## Summary

Rules that compare hand-written trait code with Derive More and protect invariants when derives create new operations.

## How to enable

Enable the `derive_more` Cargo feature to register this family. Enable the whole family with `rlib::derive_more`.

## Lints

| Lint | Summary | Purpose | Fix |
| --- | --- | --- | --- |
| [`rlib::derive_more_ambiguous_derived_error_sources`](./derive_more_ambiguous_derived_error_sources/README.md) | Finds `derive_more::Error` structs and enum variants that implicitly select a conventionally named `source` field while another error-named field also looks like a cause. | Correctness | Manual |
| [`rlib::derive_more_derived_constructors_bypassing_invariants`](./derive_more_derived_constructors_bypassing_invariants/README.md) | Finds `derive_more::Constructor` uses that expose raw field assembly around a constructor with an actual failure path. | API design | Manual |
| [`rlib::derive_more_derived_conversions_bypassing_invariants`](./derive_more_derived_conversions_bypassing_invariants/README.md) | Finds `derive_more::From` construction paths that bypass a type's fallible constructor. | API design | Manual |
| [`rlib::derive_more_inconsistent_derived_equality`](./derive_more_inconsistent_derived_equality/README.md) | Finds `derive_more::PartialEq` struct and enum configurations that use an exact field-level `skip` while a generated `Hash` or `Ord` implementation still includes that field. | Code clarity | Manual |
| [`rlib::derive_more_manual_aggregation_impls`](./derive_more_manual_aggregation_impls/README.md) | Finds owned `Sum<Self>` and `Product<Self>` implementations on tuple, named, and generic single-field newtypes that map the sole field and delegate directly to the corresponding aggregation. | API design | Manual |
| [`rlib::derive_more_manual_constructors`](./derive_more_manual_constructors/README.md) | Finds public constant `new` methods that only assign their parameters to every struct field. | API design | Manual |
| [`rlib::derive_more_manual_conversion_impls`](./derive_more_manual_conversion_impls/README.md) | Finds hand-written `From` implementations that only construct or extract the sole field of a newtype. | API design | Manual |
| [`rlib::derive_more_manual_equality_impls`](./derive_more_manual_equality_impls/README.md) | Finds field-by-field `PartialEq` implementations, with an optional marker `Eq` implementation, that compare corresponding fields and nothing else. | API design | Manual |
| [`rlib::derive_more_manual_error_impls`](./derive_more_manual_error_impls/README.md) | Finds hand-written `std::error::Error` implementations whose complete behavior is reproducible by `derive_more::Error`. | API design | Manual |
| [`rlib::derive_more_manual_formatting_impls`](./derive_more_manual_formatting_impls/README.md) | Finds standard formatting implementations that only delegate to fields, use one reproducible `write!`, or map enum variants to static text. | API design | Manual |
| [`rlib::derive_more_manual_forwarding_interfaces`](./derive_more_manual_forwarding_interfaces/README.md) | Finds `AsRef`, `AsMut`, `Deref`, `DerefMut`, `Index`, and `IndexMut` implementations that expose or forward to one stored field without adding policy. | API design | Manual |
| [`rlib::derive_more_manual_from_str_impls`](./derive_more_manual_from_str_impls/README.md) | Finds newtype `FromStr` implementations that parse the sole field and map it directly into the wrapper. | API design | Manual |
| [`rlib::derive_more_manual_into_iterator_impls`](./derive_more_manual_into_iterator_impls/README.md) | Finds owned, shared, and mutable `IntoIterator` implementations that delegate directly to the sole field of a wrapper. | API design | Manual |
| [`rlib::derive_more_manual_operator_impls`](./derive_more_manual_operator_impls/README.md) | Finds standard unary, binary, and assignment operator implementations on single-field newtypes that perform only the same-field operation exactly as derive_more does. | API design | Manual |
| [`rlib::derive_more_manual_variant_accessors`](./derive_more_manual_variant_accessors/README.md) | Finds complete hand-written `is_<variant>` predicate families that are exactly reproducible by `derive_more::IsVariant` when derive_more is the configured provider. | API design | Manual |
| [`rlib::derive_more_mutable_forwarding_bypassing_invariants`](./derive_more_mutable_forwarding_bypassing_invariants/README.md) | Finds derived `AsMut`, `DerefMut`, or `IndexMut` access that exposes restricted storage belonging to a type with a fallible constructor. | Code clarity | Manual |
| [`rlib::derive_more_non_roundtripping_derived_text_contracts`](./derive_more_non_roundtripping_derived_text_contracts/README.md) | Finds numeric newtypes whose derived `Display` adds literals or a nontransparent format while their derived `FromStr` still forwards the complete string to the numeric field parser. | Correctness | Manual |
| [`rlib::derive_more_opaque_derived_display_contracts`](./derive_more_opaque_derived_display_contracts/README.md) | Finds values with derived `Display` whose `to_string()` output is used as a standard map key. | API design | Manual |
| [`rlib::derive_more_operator_derives_bypassing_invariants`](./derive_more_operator_derives_bypassing_invariants/README.md) | Finds derive_more operator, assignment, `Sum`, and `Product` derives on restricted-field types that also expose an inherent fallible constructor. | API design | Manual |
| [`rlib::derive_more_panic_prone_derived_variant_accessors`](./derive_more_panic_prone_derived_variant_accessors/README.md) | Finds calls to derive_more-generated `unwrap_*` methods when the receiver expression does not statically construct the required enum variant. | Safety | Manual |
