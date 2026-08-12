#![allow(dead_code, unknown_lints)]

use leptos::prelude::*;

async fn load_users(organization_id: u32) -> u32 {
    organization_id
}

fn resources() {
    let organization_id = RwSignal::new(1_u32);
    let _reread = Resource::new(
        move || organization_id.get(),
        move |_| async move { load_users(organization_id.get()).await },
    );

    let _forwarded = Resource::new(
        move || organization_id.get(),
        |organization_id| async move { load_users(organization_id).await },
    );

    let refresh = RwSignal::new(false);
    let _trigger = Resource::new(
        move || refresh.get(),
        move |_| async move { load_users(organization_id.get()).await },
    );
}

fn main() {}
