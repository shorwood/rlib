# `rlib::strum_conflicting_enum_serializations`

## Summary

Finds Strum enum spellings whose parser languages overlap or whose output alias is selected only by the longest-alias fallback.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::strum` |
| Cargo feature | `strum` |
| Purpose | Correctness |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds Strum enum spellings whose parser languages overlap or whose output alias is selected only by
the longest-alias fallback.

## Why this matters

Overlapping names make variants unreachable or order-dependent, while implicit output selection can
change when an alias is added.

## Examples

### Triggers the lint

```rust,ignore
#[derive(strum::EnumString)]
enum Color {
    #[strum(serialize = "gray", ascii_case_insensitive)]
    Gray,
    #[strum(serialize = "GRAY")]
    LegacyGray,
}
```

### Use this instead

Give every parser spelling one owner and set an explicit `to_string` value when aliases coexist.

```rust,ignore
#[strum(serialize = "start", to_string = "start")]
Start
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

- [`rlib::strum_divergent_variant_name_contracts`](../strum_divergent_variant_name_contracts/README.md) — Compares advertised names with accepted aliases.
- [`rlib::strum_non_roundtripping_enum_strings`](../strum_non_roundtripping_enum_strings/README.md) — Checks that generated output parses back to the same variant.
