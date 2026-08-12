# stringly_typed_domain_function_families

## What it does

Finds groups of free functions collectively acting like an impl block for a missing
string-backed domain type. A family requires either two related behaviors or one behavior
reinforced by a raw textual field with the same normalized concept name.

## Why is this bad?

A domain represented only by repeated names has no owner for validity, normalization,
comparison, formatting, or derived behavior. Callers can pass arbitrary text, related
functions remain scattered in the module namespace, and generated code tends to add more
helpers instead of recognizing the missing concept.

## Example

```rust
fn validate_slug(slug: &str) -> Result<(), SlugError> {}
fn normalize_slug(slug: &str) -> String {}
fn slug_path(slug: &str) -> PathBuf {}
```

## Use instead


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
