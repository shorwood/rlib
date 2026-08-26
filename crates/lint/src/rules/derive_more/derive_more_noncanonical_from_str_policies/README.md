# `rlib::derive_more_noncanonical_from_str_policies`

## Summary

Finds derive_more variant parser casings that can inherit a smaller enum-level policy without changing accepted input.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::derive_more` |
| Cargo feature | `derive_more` |
| Purpose | Style |
| Default level | `warn` |
| Fix | Partial |

## What it catches

Finds repeated variant-level `from_str(rename_all = ...)` directives and overrides identical to an existing enum rule.

## Why this matters

The enum-level rule exposes the parser vocabulary while exact variant overrides remain visibly exceptional.

## Examples

### Triggers the lint

```rust,ignore
#[derive(derive_more::FromStr)]
enum Event {
    #[from_str(rename_all = "camelCase")] UserCreated,
    #[from_str(rename_all = "camelCase")] UserDeleted,
}
```

### Use this instead

```rust,ignore
#[derive(derive_more::FromStr)]
#[from_str(rename_all = "camelCase")]
enum Event { UserCreated, UserDeleted }
```

## What it skips

Hoisting is skipped when it would replace derive_more's implicit case-insensitive parsing with exact matching.

## When to turn it off

Turn this lint off when repeated exact-case rules intentionally remain local to variants.

## Settings

This lint has no behavior-specific settings.

## Known limitations

Container factoring is help-only; exact redundant standalone overrides can be removed automatically.

## Related lints

- [`rlib::derive_more_non_roundtripping_derived_text_contracts`](../derive_more_non_roundtripping_derived_text_contracts/README.md) — Finds incompatible generated formatting and parsing.
