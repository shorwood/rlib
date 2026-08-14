# bon_implicit_optional_builder_members

## What it does

Finds policy-bearing `Option<T>` arguments on Bon-generated free-function and inherent-method
builders that Bon makes omittable without an explicit policy.

## Why is this bad?

Omitting a timeout, authorization decision, limit, destination, or callback can carry domain meaning.
An implicitly optional setter makes an accidental omission indistinguishable from a deliberate choice.

## Example

```rust,ignore
#[bon::builder]
fn request(timeout: Option<Duration>, url: String) {}
```

## Use instead

Require callers to provide the optional value explicitly, document intentional omission, or model the
choice with a behavioral enum:

```rust,ignore
#[bon::builder]
fn request(#[builder(required)] timeout: Option<Duration>, url: String) {}
```
