# `rlib::derive_more_noncanonical_display_policies`

## Summary

Finds derive_more unit-variant Display names that can inherit a smaller enum-level naming policy.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::derive_more` |
| Cargo feature | `derive_more` |
| Purpose | Style |
| Default level | `warn` |
| Fix | Partial |

## What it catches

Finds static unit-variant formats and casing directives that repeat implicit, inherited, or common enum naming behavior.

## Why this matters

An enum-level Display policy shows the normal representation once and reserves variant formats for meaningful exceptions.

## Examples

### Triggers the lint

```rust,ignore
#[derive(derive_more::Display)]
enum Event {
    #[display("userCreated")] UserCreated,
    #[display("userDeleted")] UserDeleted,
}
```

### Use this instead

```rust,ignore
#[derive(derive_more::Display)]
#[display(rename_all = "camelCase")]
enum Event { UserCreated, UserDeleted }
```

## What it skips

Payload-dependent, interpolated, and shared formats that cannot be represented by `rename_all` are skipped.

## When to turn it off

Turn this lint off when each static format intentionally remains attached to its variant.

## Settings

This lint has no behavior-specific settings.

## Known limitations

Multi-variant refactors are described with help instead of an automatic edit.

## Related lints

- [`rlib::derive_more_non_roundtripping_derived_text_contracts`](../derive_more_non_roundtripping_derived_text_contracts/README.md) — Finds incompatible generated formatting and parsing.
