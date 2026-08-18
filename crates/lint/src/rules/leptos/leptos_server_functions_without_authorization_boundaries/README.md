# `rlib::leptos_server_functions_without_authorization_boundaries`

## Summary

Finds Leptos server functions whose configured sensitive calls are not preceded by configured authorization evidence or covered by an explicit endpoint marker.

## At a glance

| Field | Value |
| --- | --- |
| Group | `rlib::leptos` |
| Cargo feature | `leptos` |
| Purpose | Code clarity |
| Default level | `warn` |
| Fix | Manual |

## What it catches

Finds Leptos server functions whose configured sensitive calls are not preceded by configured
authorization evidence or covered by an explicit endpoint marker.
Calls are identified from parsed Rust expressions, so names and braces in comments or literals do
not affect the policy analysis.

## Why this matters

A Leptos server function is a publicly callable API endpoint even though it looks like a local Rust
function. Sensitive reads and mutations need visible, auditable application policy.

## Examples

### Triggers the lint

```rust,ignore
#[server]
async fn delete_account(account: AccountId) -> Result<(), ServerFnError> {
    database().delete_account(account).await?;
    Ok(())
}
```

### Use this instead

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

## What it skips

Finds Leptos server functions whose configured sensitive calls are not preceded by configured authorization evidence or covered by an explicit endpoint marker. Calls are identified from parsed Rust expressions, so names and braces in comments or literals do not affect the policy analysis.

## When to turn it off

Turn this lint off when the current structure is clearer for your team.

## Settings

| Key | Type | Default | Effect |
| --- | --- | --- | --- |
| `leptos-sensitive-call-terms` | string list | built-in list | Names call terms such as `delete` and `admin` that require an authorization check. Use `".."` to keep the built-in entries. |
| `leptos-authorization-functions` | string list | `[]` | Names project functions that perform an authorization check. |
| `leptos-protected-endpoint-attributes` | string list | `[]` | Names attributes that mark a protected endpoint. |
| `leptos-public-endpoint-attributes` | string list | `[]` | Names attributes that mark an intentionally public endpoint. |

## Known limitations

No known implementation limitations.

## Related lints

None.
