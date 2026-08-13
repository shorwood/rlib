#![feature(register_tool)]
#![allow(dead_code, unknown_lints)]
#![register_tool(rlib_lint)]

use leptos::prelude::*;

async fn delete_account(_: u64) -> Result<(), ServerFnError> {
    Ok(())
}

async fn read_private_profile() -> Result<(), ServerFnError> {
    Ok(())
}

async fn authorize_account_admin(_: u64) -> Result<(), ServerFnError> {
    Ok(())
}

#[server]
async fn unguarded(account: u64) -> Result<(), ServerFnError> {
    delete_account(account).await
}

#[server]
async fn guarded(account: u64) -> Result<(), ServerFnError> {
    authorize_account_admin(account).await?;
    delete_account(account).await
}

#[server]
async fn conditionally_guarded(account: u64) -> Result<(), ServerFnError> {
    if account == 1 {
        authorize_account_admin(account).await?;
    }
    delete_account(account).await
}

#[server]
#[rlib_lint::public_endpoint]
async fn public_profile() -> Result<(), ServerFnError> {
    read_private_profile().await
}

fn main() {}
