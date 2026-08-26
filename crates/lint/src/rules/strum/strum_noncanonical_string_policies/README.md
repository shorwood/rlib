# `rlib::strum_noncanonical_string_policies`

## Summary

Finds Strum variant strings that can inherit a smaller type-level naming policy without changing generated behavior.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::strum` |
| Cargo feature | `strum` |
| Purpose | Style |
| Default level | `warn` |
| Fix | Partial |

## What it catches

Finds repeated `serialize` values, redundant `to_string` choices, and mappings expressible through `serialize_all` and output affixes.

## Why this matters

One type-level string policy makes ordinary variants predictable and keeps aliases and exceptional output choices local.

## Examples

### Triggers the lint

```rust,ignore
#[derive(strum::Display)]
enum Event {
    #[strum(serialize = "userCreated")] UserCreated,
    #[strum(serialize = "userDeleted")] UserDeleted,
}
```

### Use this instead

```rust,ignore
#[derive(strum::Display)]
#[strum(serialize_all = "camelCase")]
enum Event { UserCreated, UserDeleted }
```

## What it skips

Mappings whose aliases, output selection, parser case sensitivity, disabled state, or payload behavior would change are skipped.

## When to turn it off

Turn this lint off when repeated variant strings are intentionally preferred over an inherited Strum policy.

## Settings

This lint has no behavior-specific settings.

## Known limitations

Container factoring is offered as help rather than a coordinated automatic edit.

## Related lints

- [`rlib::strum_conflicting_enum_serializations`](../strum_conflicting_enum_serializations/README.md) — Finds ambiguous parser and output spellings.
