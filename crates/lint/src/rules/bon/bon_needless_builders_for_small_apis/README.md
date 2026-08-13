# bon_needless_builders_for_small_apis

## What it does

Finds uncustomized private Bon function builders with only one or two required parameters.

## Why is this bad?

For a small, stable, internal signature, generated builder state and setter APIs add navigation and
compile-time cost without providing optionality, defaults, conversion, or staging value.

## Example

```rust,ignore
#[bon::builder]
fn checksum(path: PathBuf, retries: u32) -> Digest { /* ... */ }
```

## Use instead

Keep the direct call when both arguments are distinct and self-explanatory:

```rust,ignore
fn checksum(path: PathBuf, retries: u32) -> Digest { /* ... */ }
```
