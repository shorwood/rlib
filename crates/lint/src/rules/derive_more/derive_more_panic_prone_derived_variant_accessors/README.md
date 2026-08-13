# derive_more_panic_prone_derived_variant_accessors

## What it does

Finds calls to derive_more-generated `unwrap_*` methods when the receiver expression does not
statically construct the required enum variant.

## Why is this bad?

Generated unwrap accessors panic for every other variant. Calling one on a parameter or merged value
hides an unchecked variant assumption behind an ordinary-looking method call.

## Example

```rust,ignore
#[derive(derive_more::Unwrap)]
enum Response { Ok(Value), Error(Problem) }

fn render(response: Response) {
    render_value(response.unwrap_ok());
}
```

## Use instead

Use pattern matching or derive `TryUnwrap` when the active variant is not established by construction.

```rust,ignore
match response {
    Response::Ok(value) => render_value(value),
    Response::Error(problem) => render_problem(problem),
}
```
