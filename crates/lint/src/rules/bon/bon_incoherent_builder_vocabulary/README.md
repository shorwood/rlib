# bon_incoherent_builder_vocabulary

## What it does

Finds explicit Bon start, finish, or member names that replace established domain terms with generic
vocabulary.

## Why is this bad?

Names such as `process`, `execute`, or `value` force callers to rediscover meaning that the authored
operation and member names already communicate.

## Example

```rust,ignore
#[bon::builder(finish_fn = execute)]
fn publish_report(#[builder(name = data)] destination: String) {}
```

## Use instead

Use Bon's conventional defaults or preserve domain-specific terminology.

```rust,ignore
#[bon::builder]
fn publish_report(destination: String) {}
```
