# `rlib::serde_noncanonical_rename_policies`

## Summary

Finds Serde member names that can inherit a smaller container naming policy without changing the wire contract.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::serde` |
| Cargo feature | `serde` |
| Purpose | Style |
| Default level | `warn` |
| Fix | Partial |

## What it catches

Finds repeated or behavior-neutral `rename` and `alias` directives on derived Serde fields and variants, including identical directional branches and inactive private-contract branches.

## Why this matters

A container default makes the general wire-name policy visible and leaves member attributes for actual exceptions.

## Examples

### Triggers the lint

```rust,ignore
#[derive(serde::Serialize)]
enum Event {
    #[serde(rename = "userCreated")] UserCreated,
    #[serde(rename = "userDeleted")] UserDeleted,
}
```

### Use this instead

```rust,ignore
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
enum Event { UserCreated, UserDeleted }
```

## What it skips

Rewrites that change a direction, alias, skip, flatten, untagged representation, or public explicit-name policy are skipped.

## When to turn it off

Turn this lint off when leaf declarations intentionally document a protocol despite repeating its container policy.

## Settings

This lint has no behavior-specific settings.

## Known limitations

Coordinated container refactors are reported as help rather than edited automatically.

## Related lints

- [`rlib::serde_unstable_implicit_wire_names`](../serde_unstable_implicit_wire_names/README.md) — Requires explicit naming policy on public wire contracts.
