# Serde lints

## Summary

Rules for stable Serde data formats and round-trip behavior.

## How to enable

Enable the `serde` Cargo feature to register this family. Enable the whole family with `rlib::serde`.

## Lints

| Lint | Summary | Purpose | Fix |
| --- | --- | --- | --- |
| [`rlib::serde_ambiguous_untagged_enums`](./serde_ambiguous_untagged_enums/README.md) | Finds pairs of named-field variants in a derived `#[serde(untagged)]` enum for which a concrete input object can deserialize as either variant. | Code clarity | Manual |
| [`rlib::serde_asymmetric_serde_contracts`](./serde_asymmetric_serde_contracts/README.md) | Finds undocumented directional Serde renames whose serialization and deserialization names differ on a type deriving both traits. | Correctness | Manual |
| [`rlib::serde_catch_all_variants_hiding_schema_drift`](./serde_catch_all_variants_hiding_schema_drift/README.md) | Finds `#[serde(other)]` variants in public enums with derived deserialization. | Code clarity | Manual |
| [`rlib::serde_defaults_hiding_missing_data`](./serde_defaults_hiding_missing_data/README.md) | Finds undocumented bare `#[serde(default)]` fields whose type is not `Option`. | Code clarity | Manual |
| [`rlib::serde_deserialization_bypassing_invariants`](./serde_deserialization_bypassing_invariants/README.md) | Finds derived `Deserialize` on restricted-field types that also expose an inherent fallible direct constructor. | Code clarity | Manual |
| [`rlib::serde_duplicate_serialized_names`](./serde_duplicate_serialized_names/README.md) | Finds fields or variants whose effective Serde names collide after directional renames, aliases, container case conversion, and directional skips. | Correctness | Manual |
| [`rlib::serde_flattened_field_collisions`](./serde_flattened_field_collisions/README.md) | Finds concrete wire-name collisions between ordinary fields and locally visible struct schemas merged with `#[serde(flatten)]`. | Code clarity | Manual |
| [`rlib::serde_flattened_unknown_field_policies`](./serde_flattened_unknown_field_policies/README.md) | Finds derived Serde deserializers that combine a flattened field with `#[serde(deny_unknown_fields)]` on the same container. | Code clarity | Manual |
| [`rlib::serde_format_specific_serde_impls`](./serde_format_specific_serde_impls/README.md) | Finds undocumented manual Serde implementations that depend on a specific format or use different data shapes when `is_human_readable()` changes. | API design | Manual |
| [`rlib::serde_lossy_conditional_serialization`](./serde_lossy_conditional_serialization/README.md) | Finds conditionally serialized required fields that have neither optionality nor a deserialization default. | Safety | Manual |
| [`rlib::serde_manual_deserialize_impls`](./serde_manual_deserialize_impls/README.md) | Finds `Deserialize` implementations on single-field newtypes that decode the field unchanged and immediately construct `Self`. | API design | Manual |
| [`rlib::serde_manual_serialize_impls`](./serde_manual_serialize_impls/README.md) | Finds `Serialize` implementations on single-field newtypes that forward the sole field unchanged to the provided serializer. | API design | Manual |
| [`rlib::serde_noncanonical_rename_policies`](./serde_noncanonical_rename_policies/README.md) | Finds Serde member names that can inherit a smaller container naming policy without changing the wire contract. | Style | Partial |
| [`rlib::serde_non_roundtripping_serde_adapters`](./serde_non_roundtripping_serde_adapters/README.md) | Finds paired `serialize_with` and `deserialize_with` helpers whose names identify the same adapter family but different units or encodings. | Correctness | Manual |
| [`rlib::serde_remote_representations_drifting_from_sources`](./serde_remote_representations_drifting_from_sources/README.md) | Finds undocumented Serde remote representations that omit fields from a uniquely resolved local source struct. | API design | Manual |
| [`rlib::serde_sensitive_fields_serialized_by_default`](./serde_sensitive_fields_serialized_by_default/README.md) | Finds public derived Serde serializers that include strongly named credential fields stored in raw string or byte carrier types without an explicit serialization policy. | Safety | Manual |
| [`rlib::serde_unstable_implicit_wire_names`](./serde_unstable_implicit_wire_names/README.md) | Finds public Serde types whose derived field or variant names are used in serialized data without being stated explicitly. | Code clarity | Manual |
