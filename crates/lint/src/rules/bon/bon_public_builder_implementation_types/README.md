# bon_public_builder_implementation_types

## What it does

Finds generated Bon builder or generated builder state definitions named in hand-written public function, method, or
field types.

## Why is this bad?

Generated types encode macro representation and required-member state. Publishing them makes a
macro upgrade or builder-policy change part of the library's compatibility contract.

## Example

```rust,ignore
#[derive(bon::Builder)]
pub struct Request { host: String }

pub fn partially_configured() -> RequestBuilder { Request::builder() }
```

## Use instead

Expose completed domain values or define an intentional domain abstraction for staged construction.

```rust,ignore
pub fn configured_request() -> Request {
    Request::builder().host("example.test".to_owned()).build()
}
```
