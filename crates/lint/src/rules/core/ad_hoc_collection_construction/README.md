# `rlib::ad_hoc_collection_construction`

## Summary

Finds free and inherent constructors that completely consume an iterable into a local collection wrapper, plus free and inherent extension operations that do the same through a mutable target.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::core` |
| Cargo feature | always |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds free and inherent constructors that completely consume an iterable into a local
collection wrapper, plus free and inherent extension operations that do the same through a
mutable target. Source items must flow into target-owned standard collection storage.

## Why this matters

Project-specific collection protocols cannot use `collect`, `extend`, generic bounds, or
standard iterator composition. Several names for the same item family also conceal whether
ordering, duplicates, validation, and truncation are intrinsic or accidental.

## Examples

### Triggers the lint

```rust
struct Report { entries: Vec<Entry> }
# struct Entry;
impl Report {
    fn from_entries(entries: impl IntoIterator<Item = Entry>) -> Self {
        let mut report = Self { entries: Vec::new() };
        report.entries.extend(entries);
        report
    }
}
```

### Use this instead

Fallible validation, filtering, transformation, truncation, early exit, explicit policy
arguments, and builder finalization remain named. Multiple distinct item types may each
implement the standard traits independently.

```rust
impl FromIterator<Entry> for Report {
    fn from_iter(entries: impl IntoIterator<Item = Entry>) -> Self {
        Self { entries: entries.into_iter().collect() }
    }
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
