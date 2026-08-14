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

fn used_argument_still_rereads() {
    let organization_id = RwSignal::new(1_u32);
    let _resource = Resource::new(
        move || organization_id.get(),
        move |tracked| async move { load_users(tracked + organization_id.get()).await },
    );
}

fn blocking_untracked_reread() {
    let organization_id = RwSignal::new(1_u32);
    let _resource = Resource::new_blocking(
        move || organization_id.get(),
        move |_| async move { load_users(organization_id.get_untracked()).await },
    );
}

#[derive(Clone, Copy)]
struct Sources {
    organization_id: RwSignal<u32>,
    refresh: RwSignal<bool>,
}

fn projected_reread() {
    let sources = Sources {
        organization_id: RwSignal::new(1),
        refresh: RwSignal::new(false),
    };
    let _resource = Resource::new(
        move || sources.organization_id.with(|id| *id),
        move |_| async move { load_users(sources.organization_id.try_get().unwrap_or(0)).await },
    );
}

fn different_projection_is_independent() {
    let sources = Sources {
        organization_id: RwSignal::new(1),
        refresh: RwSignal::new(false),
    };
    let _resource = Resource::new(
        move || sources.refresh.get(),
        move |_| async move { load_users(sources.organization_id.get()).await },
    );
}

fn untracked_source_is_not_a_dependency() {
    let organization_id = RwSignal::new(1_u32);
    let _resource = Resource::new(
        move || organization_id.get_untracked(),
        move |_| async move { load_users(organization_id.get()).await },
    );
}

fn main() {}
