# bon_parameter_heavy_apis_without_builders

## What it does

Finds public Rust APIs with enough positional parameters and ambiguity that a Bon named builder
would materially improve calls.

## Why is this bad?

Long positional calls make boolean policy and same-representation domain values easy to transpose.
Callers must repeatedly reconstruct parameter meaning from the declaration.

## Example

```rust,ignore
pub fn render(path: PathBuf, width: u32, height: u32, cache: bool, color: bool) {}
```

## Use instead

Use Bon when the values are independent call-time choices; prefer a named domain options type when
they form a reusable concept:

```rust,ignore
#[bon::builder]
pub fn render(path: PathBuf, width: u32, height: u32, cache: bool, color: bool) {}
```
