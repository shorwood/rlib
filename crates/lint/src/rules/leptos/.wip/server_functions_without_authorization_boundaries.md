# Server functions without authorization boundaries

## Proposition

Add a configurable `server_functions_without_authorization_boundaries` lint for Leptos server
functions that expose sensitive reads or mutations without an explicit, auditable authorization
boundary.

```rust
// Suspicious: this is a public endpoint despite looking like a local function.
#[server]
async fn delete_account(account: AccountId) -> Result<(), ServerFnError> {
    database().delete_account(account).await?;
    Ok(())
}
```

```rust
// Better: endpoint policy is visible before the mutation.
#[server]
async fn delete_account(account: AccountId) -> Result<(), ServerFnError> {
    let principal = require_authenticated_principal().await?;
    authorize_account_admin(&principal, account).await?;
    database().delete_account(account).await?;
    Ok(())
}
```

Leptos server functions compile into publicly callable API endpoints. The official guide emphasizes
that callers must implement authentication, authorization, encryption, rate limiting, and other
ordinary API security controls as appropriate.

Reference: [Server Functions](https://book.leptos.dev/server/25_server_functions.html).

## Why configuration is required

Rust type and body analysis cannot prove arbitrary authorization policy. Public endpoints, login
functions, health checks, and intentionally anonymous reads may require no authenticated principal.
Projects also obtain identity through different extractors, contexts, services, and middleware.

This lint should therefore enforce visible policy evidence configured by the project rather than
claiming to solve authorization automatically.

## Conservative detection

- Inspect authored top-level functions annotated with Leptos's `#[server]` macro.
- Classify mutating behavior through configured database, repository, command, and external-service
  calls.
- Increase severity for delete, update, create, upload, administrative, credential, and private-data
  vocabulary.
- Accept configured authentication extractors, authorization helper calls, middleware marker
  attributes, or an explicit public-endpoint marker.
- Require authorization evidence to dominate the sensitive operation on every control-flow path.
- Diagnose at the endpoint declaration and list unguarded sensitive operations.

## Intentional public endpoints

An explicit marker makes review intent visible:

```rust
#[server]
#[public_endpoint]
async fn health() -> Result<Health, ServerFnError> {
    // ...
}
```

The marker should be configured by the application and ideally checked by its routing or middleware
infrastructure. A lint-only comment suppression is weaker because it does not become part of the
endpoint's architectural vocabulary.

## Context caveat

Server functions invoked during SSR and through standalone endpoints may not receive identical
application contexts unless routing integration provides them explicitly. Authentication evidence
based on `expect_context` must therefore refer to a context known to exist for endpoint calls, not
merely component rendering.

Reference: [Asynchronous Closures and Futures](https://book.leptos.dev/server/28_async_quick_reference.html).

## Open decisions

- Which project configuration declares authentication and authorization evidence.
- Whether every server function needs an explicit public-or-protected marker.
- How middleware-protected route groups are associated with generated server-function endpoints.
- Whether rate limiting, CSRF protection, and authorization should be separate rules.
- Whether sensitive return types can be inferred strongly enough to protect reads as well as writes.
