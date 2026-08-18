# `rlib::framework_resolution_required`

## Summary

Warns when several enabled libraries can generate the same code and the project has not chosen which one to use.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::framework` |
| Cargo feature | `framework` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Warns when several enabled libraries can generate the same code and the project has not chosen
which one to use.

## Why this matters

The libraries may produce similar code today while exposing different traits, helper methods, and
upgrade paths. Choosing whichever dependency happens to be present makes public behavior change for
reasons unrelated to the source being linted.

## Examples

### Triggers the lint

```rust,ignore
impl Stage {
    const ALL: [Self; 2] = [Self::Planned, Self::Complete];
}
```

### Use this instead

Select the intended provider explicitly in the workspace-root `dylint.toml`:

```rust,ignore
[rlib-lint]
enum-variant-collection-provider = "strum_variant_array"
enum-variant-predicate-provider = "strum_enum_is"
enum-display-provider = "strum_display"
enum-string-parsing-provider = "strum_enum_string"
error-implementation-provider = "thiserror_error"
error-variant-conversion-provider = "thiserror_from"
```

Only configure choices that apply to the enabled libraries. The lint does not pick a provider or
silently prefer one based on dependency order.

## What it skips

No additional exclusions are documented.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `error-implementation-provider` | string | not set | Chooses the crate that generates `Error`. Values: `derive_more_error`, `thiserror_error`. |
| `error-variant-conversion-provider` | string | not set | Chooses the crate that generates conversions into error variants. Values: `derive_more_from`, `thiserror_from`. |
| `enum-variant-collection-provider` | string | not set | Chooses the generated enum collection. Values: `strum_enum_iter`, `strum_variant_array`. |
| `enum-variant-predicate-provider` | string | not set | Chooses the crate that generates enum variant checks. Values: `derive_more_is_variant`, `strum_enum_is`. |
| `enum-string-parsing-provider` | string | not set | Chooses the crate that generates string parsing. Values: `derive_more_from_str`, `strum_enum_string`. |
| `enum-display-provider` | string | not set | Chooses the crate that generates display text. Values: `derive_more_display`, `strum_display`. |

## Known limitations

No known implementation limitations.

## Related lints

None.
