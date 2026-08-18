# `rlib::stringly_typed_domain_function_families`

## Summary

Finds groups of free functions collectively acting like an impl block for a missing string-backed domain type.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds groups of free functions collectively acting like an impl block for a missing
string-backed domain type. A family requires either two related behaviors or one behavior
reinforced by a raw textual field with the same normalized concept name.

## Why this matters

A domain represented only by repeated names has no owner for validity, normalization,
comparison, formatting, or derived behavior. Callers can pass arbitrary text, related
functions remain scattered in the module namespace, and generated code tends to add more
helpers instead of recognizing the missing concept.

## Examples

### Triggers the lint

```rust
fn validate_slug(slug: &str) -> Result<(), SlugError> {}
fn normalize_slug(slug: &str) -> String {}
fn slug_path(slug: &str) -> PathBuf {}
```

### Use this instead

Introduce one domain value that owns the invariant and behavior:

```rust
struct Slug(String);

impl Slug {
    fn path(&self) -> PathBuf {}
}

impl FromStr for Slug {
    type Err = SlugError;
    fn from_str(source: &str) -> Result<Self, Self::Err> {}
}
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
