# bon_inconsistent_builder_conversions

## What it does

Finds same-typed members of one Bon function builder that arbitrarily mix `#[builder(into)]` with
strict input types.

## Why is this bad?

Callers must remember which otherwise equivalent setters accept owned conversions. This makes a
generated named API less predictable than the positional function it replaces.

## Example

```rust,ignore
#[bon::builder]
fn request(#[builder(into)] url: String, header: String) {}
```

## Use instead

Apply one deliberate conversion policy to the same representation, or use a custom converter where
the members genuinely have different validation semantics:

```rust,ignore
#[bon::builder]
fn request(#[builder(into)] url: String, #[builder(into)] header: String) {}
```
