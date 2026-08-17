# function_local_items

## What it does

Rejects authored item declarations inside functions, methods, closures, and their nested blocks.
This includes local types and impls, helper functions, constants and statics, imports, modules, and
macro definitions. Items produced by macro expansion are ignored.

## Why is this bad?

Local items mix structural declarations with executable control flow, conceal reusable helpers, and
make their ownership harder to discover. Module or associated scope gives declarations a stable,
searchable home.

## Example

```rust
fn render() {
    struct Renderer;
    impl Renderer {}
}
```

## Use instead

```rust
struct Renderer;

impl Renderer {}

fn render() {}
```
