# `rlib::unnecessarily_broad_visibility`

## Summary

Finds hand-written module items, types, functions, constants, statics, struct and union fields, and inherent associated items whose standard visibility is broader than every resolved use in the current crate.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Needs review |

## What it catches

Finds hand-written module items, types, functions, constants, statics, struct and union fields,
and inherent associated items whose standard visibility is broader than every resolved use
in the current crate. Named and positional field uses, including tuple construction and
destructuring, contribute to the required boundary. It recommends private visibility for uses
confined to the defining module, `pub(super)` for the immediate parent subtree, and
`pub(crate)` for wider crate use.

Unrestricted `pub` APIs in publishable library packages are preserved because downstream
users cannot be observed. Binaries and packages explicitly marked `publish = false` are
treated as closed. Missing or malformed manifest metadata is interpreted conservatively.
Hand-written reach is capped by restricted ancestor modules, so `pub` inside a private namespace
is accepted when that namespace already supplies the narrowest effective boundary. Generated
declarations, foreign entry points, proc macros, and language items are ignored.

## Why this matters

Broad visibility is a dependency permission, not decoration. An unnecessarily visible item
allows code to bypass its owning module, grows the public API, and makes later
relocation or invariant enforcement harder. Generated code often writes `pub` before it has
decided where an operation belongs; the resulting API then outlives that accident.

## Examples

### Triggers the lint

```rust
mod normalization {
    pub fn normalize_internal_key(key: &str) -> String {
        key.trim().to_owned()
    }
}
```

### Use this instead

If every caller is inside `normalization`, keep ownership local:

```rust
mod normalization {
    fn normalize_internal_key(key: &str) -> String {
        key.trim().to_owned()
    }
}
```

Suggestions are deliberately `MaybeIncorrect`: inactive `cfg` branches and Rust consumers
outside a closed workspace may be invisible to the current compiler invocation.

## What it skips

Generated declarations, foreign entry points, proc macros, and language items are ignored.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

- [`rlib::noncanonical_restricted_visibility`](../noncanonical_restricted_visibility/README.md) — Limits the forms used to express visibility.
- [`rlib::visibility_required_only_by_tests`](../visibility_required_only_by_tests/README.md) — Separates production visibility from test access.
