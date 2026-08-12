# positional_aggregate_fields

## What it does

Finds tuple structs and tuple-like enum variants with two or more fields. Unit forms and
single-field newtypes remain valid because they do not present multiple unnamed roles.

## Why is this bad?

Naming an aggregate does not make its individual positions self-explanatory. Constructors
and pattern matches still rely on ordering, so readers and agents must repeatedly infer the
same roles. Record fields make the domain vocabulary explicit at every use site.

For example, these declarations name the aggregate but not its components:

## Example

```rust
struct Rename(Span, String);

enum Finding {
    Replacement(Span, String),
}
```

## Use instead


Use record fields that state what each value means:

```rust
struct Rename {
    span: Span,
    replacement: String,
}

enum Finding {
    Replacement { span: Span, replacement: String },
}
```
