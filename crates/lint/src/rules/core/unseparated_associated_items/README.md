# unseparated_associated_items

## What it does

Requires adjacent items in implementation and trait blocks to have one visually empty line
between them. Documentation and outer attributes belong to the following item, so the blank
line must appear before that leading context. Macro-generated declarations are ignored.

When insertion preserves whitespace, documentation, and attributes, the lint offers a
machine-applicable correction. An ordinary comment in the boundary still produces guidance,
but its ownership is left for the author to resolve rather than being changed automatically.

## Why is this bad?

Dense associated-item blocks hide where one contract or operation ends and the next begins.
Stable visual boundaries make implementations easier to scan, reduce accidental reading of
one item's documentation as another item's context, and give automated editors a predictable
representation without forcing unrelated module declarations apart.

For example, these methods run together:

## Example

```rust
struct Report;

impl Report {
    fn title(&self) -> &str { "summary" }
    fn render(&self) -> String { self.title().to_owned() }
}
```

## Use instead

Separate each associated item with one empty line:

```rust
struct Report;

impl Report {
    fn title(&self) -> &str { "summary" }

    fn render(&self) -> String { self.title().to_owned() }
}
```
