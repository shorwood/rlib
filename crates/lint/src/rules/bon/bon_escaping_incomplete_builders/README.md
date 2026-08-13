# bon_escaping_incomplete_builders

## What it does

Finds generated Bon builders returned from private functions or stored in authored fields instead of
being completed within a focused construction expression.

## Why is this bad?

Escaped typestate obscures which scope owns remaining required members and couples unrelated code to
the order and representation of staged construction.

## Example

```rust,ignore
fn base_request() -> RequestBuilder {
    Request::builder()
}
```

## Use instead

Complete construction locally, or define a named domain state when partial application is an
intentional contract.

```rust,ignore
fn base_request() -> Request {
    Request::builder().host("example.test".to_owned()).build()
}
```
