# `rlib::strum_divergent_variant_name_contracts`

## Summary

Finds enums whose advertised `VariantNames` spelling differs from an alias accepted by `EnumString`.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::strum` |
| Cargo feature | `strum` |
| Purpose | Correctness |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds enums whose advertised `VariantNames` spelling differs from an alias accepted by
`EnumString` for the same variant.

## Why this matters

Callers commonly use `VARIANTS` as the advertised set of accepted values. Hidden aliases make that
list incomplete and can produce inconsistent validation or help text.

## Examples

### Triggers the lint

```rust,ignore
#[derive(strum::VariantNames, strum::EnumString)]
enum Color { #[strum(serialize = "gray", serialize = "grey")] Gray }
```

### Use this instead

Expose a deliberate standard-name API and document aliases separately.

```rust,ignore
const CANONICAL_NAMES: &[&str] = &["pending", "complete"];
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off only when the reported behavior is intentional and covered by tests.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

- [`rlib::strum_conflicting_enum_serializations`](../strum_conflicting_enum_serializations/README.md) — Finds overlapping or unclear accepted spellings.
- [`rlib::strum_non_roundtripping_enum_strings`](../strum_non_roundtripping_enum_strings/README.md) — Checks that output parses back to the same variant.
