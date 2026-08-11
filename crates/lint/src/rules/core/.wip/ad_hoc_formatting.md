# Ad hoc formatting

## Proposition

Add an `ad_hoc_formatting` lint for methods that reproduce a type's canonical human-readable
`Display` representation through project-specific string-returning methods.

```rust
// Suspicious when this is UserId's one ordinary presentation.
impl UserId {
    fn display_value(&self) -> String {}
    fn to_text(&self) -> String {}
}
```

```rust
// Better for one canonical human-readable representation.
impl Display for UserId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        // Write without forcing an intermediate allocation.
    }
}
```

`Display` integrates with formatting machinery, error reporting, logs, interpolation, and generic
APIs. Its formatter-based contract can also avoid the allocation imposed by returning `String`.

## Relationship with Clippy

Clippy already diagnoses the exact inherent `to_string(&self) -> String` pattern. This proposal
should not duplicate that spelling check. It targets structurally equivalent methods with other
names and families of free formatting functions that collectively establish one canonical
presentation.

## Conservative detection

- Inspect inherent methods taking `&self` and returning `String`, `Cow<str>`, or an infallible
  textual wrapper.
- Inspect free functions taking one reference to a local type and returning text.
- Increase confidence for `display`, `format`, `format_*`, `render_text`, `to_text`, and type-family
  naming.
- Increase confidence when the body formats stable identity-bearing fields without configuration.
- Increase confidence when call sites immediately pass the result to formatting or logging APIs.
- Check for an existing `Display` impl and diagnose redundant competing presentation methods.

## Formats and encodings are not Display

```rust
document.render_html()
token.encode_base64()
manifest.serialize_canonical_json()
timestamp.format_with(pattern)
diagnostic.render_ansi(theme)
```

These operations expose a format, encoding, configuration, or transport representation. Their names
are essential and should remain explicit. `Display` is appropriate only when users can reasonably
expect one ordinary representation without supplying policy.

## Sensitive and ambiguous representations

Secret-bearing types should not be encouraged to implement `Display` because ordinary formatting
could leak credentials:

```rust
access_token.expose_secret()
password.expose_secret()
```

Types with several peer representations may deliberately omit `Display`:

```rust
Color::to_hex()
Color::to_css_rgb()
Color::to_css_hsl()
```

The lint should inspect type and method vocabulary for secrets, explicit formats, and configuration
before recommending a trait.

## Interaction with parsing

A string-backed type implementing `FromStr` often benefits from a corresponding `Display` whose
output parses back to an equivalent value. That round-trip is useful evidence, not an unconditional
law for every displayable type. If both directions are canonical, diagnostics can describe the
expected relationship explicitly.

## Open decisions

- What evidence proves that a representation is canonical rather than merely common.
- Whether a documented string-returning convenience method may coexist with `Display`.
- Whether no-allocation accessors such as `as_str` should always remain exempt.
- Which secret-bearing names and marker traits suppress a `Display` recommendation.
- Whether serialization-derived formatting should count as evidence against canonical display.
