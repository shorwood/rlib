# bon_redundant_positional_and_builder_apis

## What it does

Finds complex externally reachable Bon function builders that keep most inputs positional through
exact member-level `start_fn` or `finish_fn` configuration.

## Why is this bad?

The builder adds generated API surface without providing a substantially named call contract. Long
positional start and finish calls retain the ordering mistakes that the builder should prevent.

## Example

```rust,ignore
#[bon::builder]
pub fn render(
    #[builder(start_fn)] path: PathBuf,
    #[builder(start_fn)] width: u32,
    #[builder(start_fn)] height: u32,
    #[builder(finish_fn)] cache: bool,
    color: bool,
) {}
```

## Use instead

Keep only a small identity-bearing prefix positional and make the rest named setters.

```rust,ignore
#[bon::builder]
pub fn render(#[builder(start_fn)] path: PathBuf, width: u32, height: u32, cache: bool, color: bool) {}
```
