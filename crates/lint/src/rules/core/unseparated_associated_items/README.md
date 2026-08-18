# `rlib::unseparated_associated_items`

## Summary

Requires adjacent items in implementation and trait blocks to have one visually empty line between them.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Style |
| Default level | `warn` |
| Fix | Automatic |

## What it catches

Requires adjacent items in implementation and trait blocks to have one visually empty line
between them. Documentation and outer attributes belong to the following item, so the blank
line must appear before that leading context. Macro-generated declarations are ignored.

When insertion preserves whitespace, documentation, and attributes, the lint offers a
machine-applicable correction. An ordinary comment in the boundary still produces guidance,
but its ownership is left for the author to resolve rather than being changed automatically.

## Why this matters

Dense associated-item blocks hide where one operation ends and the next begins.
Stable visual boundaries make implementations easier to scan, reduce accidental reading of
one item's documentation as another item's context, and give automated editors a predictable
representation. Use `unseparated_module_items` for declarations directly owned
by a module.

For example, these methods run together:

## Examples

### Triggers the lint

```rust
struct Report;

impl Report {
    fn title(&self) -> &str { "summary" }
    fn render(&self) -> String { self.title().to_owned() }
}
```

### Use this instead

Separate each associated item with one empty line:

```rust
struct Report;

impl Report {
    fn title(&self) -> &str { "summary" }

    fn render(&self) -> String { self.title().to_owned() }
}
```

## What it skips

Macro-generated declarations are ignored.

## When to turn it off

Turn this lint off when your project deliberately follows a different style.

## Settings

This lint has no behavior-specific settings.

## Known limitations

No known implementation limitations.

## Related lints

None.
