# boolean_function_arguments

## What it does

Finds direct boolean function and method parameters. One boolean is accepted only for an
exact setter contract such as `set_enabled(enabled: bool)`; two or more are always rejected.
Boolean returns, predicate callbacks, and wrapped state such as `Option<bool>` are outside
the rule.

## Why is this bad?

A boolean argument hides a choice at its call site. Literals such as `true` and `false` do
not explain the policy they select, and multiple flags can be reordered without a type
error. Adding another boolean is an easy compiling change for an agent but steadily erodes
the API's vocabulary.

This call makes neither decision reviewable:

## Example

```rust
fn render(document: &Document, minify: bool, include_metadata: bool) {}

render(&document, true, false);
```

## Use instead

Name the policy through an enum or options type:

```rust
struct RenderOptions {
    minify: bool,
    include_metadata: bool,
}

fn render(document: &Document, options: RenderOptions) {}
```
