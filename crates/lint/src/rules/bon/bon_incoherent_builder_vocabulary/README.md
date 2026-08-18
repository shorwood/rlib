# `rlib::bon_incoherent_builder_vocabulary`

## Summary

Finds explicit Bon start, finish, or member names that replace established domain terms with generic names on free-function builders, associated-function builders, and derived struct builders.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::bon` |
| Cargo feature | `bon` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds explicit Bon start, finish, or member names that replace established domain terms with
generic names on free-function builders, associated-function builders, and derived
struct builders.

## Why this matters

Names such as `process`, `execute`, or `value` force callers to rediscover meaning that the hand-written
operation and member names already communicate.

## Examples

### Triggers the lint

```rust,ignore
#[bon::builder(finish_fn = execute)]
fn publish_report(#[builder(name = data)] destination: String) {}
```

### Use this instead

Use Bon's conventional defaults or preserve domain-specific terminology.

```rust,ignore
#[bon::builder]
fn publish_report(destination: String) {}
```

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
