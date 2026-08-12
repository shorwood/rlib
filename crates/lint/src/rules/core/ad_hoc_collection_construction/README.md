# ad_hoc_collection_construction

## What it does

Finds free and inherent constructors that completely consume an iterable into a local
collection wrapper, plus free and inherent extension operations that do the same through a
mutable target. Source items must flow into target-owned standard collection storage.

## Why is this bad?

Project-specific collection protocols cannot use `collect`, `extend`, generic bounds, or
standard iterator composition. Several names for the same item family also conceal whether
ordering, duplicates, validation, and truncation are intrinsic or accidental.

## Example

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

## Use instead

Fallible validation, filtering, truncation, early exit, explicit policy arguments, and
builder finalization remain named. Multiple distinct item types may each implement the
standard traits independently.

```rust
impl FromIterator<Entry> for Report {
    fn from_iter(entries: impl IntoIterator<Item = Entry>) -> Self {
        Self { entries: entries.into_iter().collect() }
    }
}
```
