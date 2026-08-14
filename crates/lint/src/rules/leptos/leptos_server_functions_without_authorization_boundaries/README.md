# leptos_server_functions_without_authorization_boundaries

## What it does

Finds Leptos server functions whose configured sensitive calls are not preceded by configured
authorization evidence or covered by an explicit endpoint marker.
Calls are identified from parsed Rust expressions, so names and braces in comments or literals do
not affect the policy analysis.

## Why is this bad?

A Leptos server function is a publicly callable API endpoint even though it looks like a local Rust
function. Sensitive reads and mutations need visible, auditable application policy.

## Example

```rust,ignore
#[server]
async fn delete_account(account: AccountId) -> Result<(), ServerFnError> {
    database().delete_account(account).await?;
    Ok(())
}
```

## Use instead

Configure the project's authorization helpers and endpoint marker attributes, then establish the
boundary before the operation:

```rust,ignore
#[server]
async fn delete_account(account: AccountId) -> Result<(), ServerFnError> {
    authorize_account_admin(account).await?;
    database().delete_account(account).await?;
    Ok(())
}
```
