# `rlib::strum_non_roundtripping_enum_strings`

## Summary

Finds unit variants whose Strum-generated output spelling is not parsed back into the same variant.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::strum` |
| Cargo feature | `strum` |
| Purpose | Correctness |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds unit variants whose Strum-generated output spelling is not parsed back into the same variant.

## Why this matters

Types that both parse and produce text usually imply a round trip. Prefixes, suffixes,
aliases, and case-insensitive collisions can silently break it.

## Examples

### Triggers the lint

```rust,ignore
#[derive(strum::Display, strum::EnumString)]
#[strum(prefix = "status-")]
enum Status { Ready }
```

### Use this instead

Align the parser and output spellings, or expose only the intentionally one-way operation.

```rust,ignore
#[strum(serialize = "pending", to_string = "pending")]
Pending
```

## What it skips

Finds unit variants whose Strum-generated output spelling is not parsed back into the same variant.

## When to turn it off

Turn this lint off only when the reported behavior is intentional and covered by tests.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

- [`rlib::strum_conflicting_enum_serializations`](../strum_conflicting_enum_serializations/README.md) — Finds overlapping or unclear accepted spellings.
- [`rlib::strum_divergent_variant_name_contracts`](../strum_divergent_variant_name_contracts/README.md) — Compares advertised names with accepted aliases.
