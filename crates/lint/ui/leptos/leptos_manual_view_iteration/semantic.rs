#![feature(register_tool)]
#![allow(
    dead_code,
    deprecated,
    rlib::leptos_boolean_component_props,
    rlib::leptos_implicit_default_component_props,
    rlib::leptos_manual_resource_refetch_signals,
    rlib::leptos_needlessly_cloned_signal_values,
    rlib::leptos_reactive_writes_during_view_construction,
    rlib::leptos_unsanitized_inner_html,
    rlib::leptos_writable_signal_component_props,
    unknown_lints
)]
#![register_tool(rlib_lint)]

use leptos::prelude::*;

#[component]
fn ReactiveMapped() -> impl IntoView {
    let (names, _) = signal(vec![String::from("Ada")]);
    view! {
        <ul>
            {move || names.get()
                .into_iter()
                .map(|name| view! { <li>{name}</li> })
                .collect_view()}
        </ul>
    }
}

#[component]
fn StaticMapped() -> impl IntoView {
    view! {
        <ul>
            {["Ada", "Grace"]
                .into_iter()
                .map(|name| view! { <li>{name}</li> })
                .collect_view()}
        </ul>
    }
}

#[component]
fn FallibleFiltered() -> impl IntoView {
    let (names, _) = signal(vec![String::from("Ada")]);
    view! {
        {move || names.try_get()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|name| (!name.is_empty()).then(|| view! { <span>{name}</span> }))
            .collect_view()}
    }
}

#[component]
fn BorrowedClone() -> impl IntoView {
    let (names, _) = signal(vec![String::from("Ada")]);
    view! {
        {move || names.with(Clone::clone)
            .into_iter()
            .map(|name| view! { <span>{name}</span> })
            .collect_view()}
    }
}

#[component]
fn ExplicitlyUntracked() -> impl IntoView {
    let (names, _) = signal(vec![String::from("Ada")]);
    view! {
        {names.get_untracked()
            .into_iter()
            .map(|name| view! { <span>{name}</span> })
            .collect_view()}
    }
}

#[component]
fn MappingBeforeAnotherAdapter() -> impl IntoView {
    view! {
        {["Ada", "Grace"]
            .into_iter()
            .map(|name| view! { <span>{name}</span> })
            .take(1)
            .collect_view()}
    }
}

#[component]
fn OtherMappingAdapters() -> impl IntoView {
    view! {
        {["Ada"]
            .into_iter()
            .flat_map(|name| [view! { <span>{name}</span> }])
            .collect_view()}
        {["Grace"]
            .into_iter()
            .map_while(|name| Some(view! { <span>{name}</span> }))
            .collect_view()}
        {["Lin"]
            .into_iter()
            .scan((), |_, name| Some(view! { <span>{name}</span> }))
            .collect_view()}
    }
}

struct Section {
    title: &'static str,
    routes: &'static [&'static str],
}

static SECTIONS: &[Section] = &[Section {
    title: "Guides",
    routes: &["/getting-started", "/components"],
}];

#[component]
fn NestedStaticMapped() -> impl IntoView {
    view! {
        {SECTIONS
            .iter()
            .map(|section| {
                view! {
                    <section>
                        <h2>{section.title}</h2>
                        {section
                            .routes
                            .iter()
                            .filter_map(|route| (!route.is_empty()).then_some(*route))
                            .map(|route| view! { <a href=route>{route}</a> })
                            .collect_view()}
                    </section>
                }
            })
            .collect_view()}
    }
}

#[component]
fn DeclarativeFor() -> impl IntoView {
    let names = RwSignal::new(vec![String::from("Ada"), String::from("Grace")]);
    view! {
        <For
            each=move || names.get()
            key=String::clone
            children=|name| view! { <span>{name}</span> }
        />
    }
}

#[component]
fn AlreadyProducedViews() -> impl IntoView {
    let ada = view! { <span>"Ada"</span> };
    let grace = view! { <span>"Grace"</span> };
    view! { {[ada, grace].into_iter().collect_view()} }
}

#[component]
fn DifferentCollectionTerminal() -> impl IntoView {
    let _views = ["Ada", "Grace"]
        .into_iter()
        .map(|name| view! { <span>{name}</span> })
        .collect::<Vec<_>>();
    view! { <span>"done"</span> }
}

struct Wrapped<I>(I);

impl<I: Iterator> Wrapped<I> {
    fn map<B, F>(self, transform: F) -> std::iter::Map<I, F>
    where
        F: FnMut(I::Item) -> B,
    {
        self.0.map(transform)
    }
}

struct ForeignCollector<I>(I);

impl<I> ForeignCollector<I> {
    fn collect_view(self) {}
}

trait WrapForeignCollector: Sized {
    fn foreign(self) -> ForeignCollector<Self> {
        ForeignCollector(self)
    }
}

impl<I> WrapForeignCollector for I {}

trait WrapIterator: Iterator + Sized {
    fn wrapped(self) -> Wrapped<Self> {
        Wrapped(self)
    }
}

impl<I: Iterator> WrapIterator for I {}

#[component]
fn InherentMapIsNotIteratorMap() -> impl IntoView {
    let (names, _) = signal(vec![String::from("Ada")]);
    view! {
        {move || names.get()
            .into_iter()
            .wrapped()
            .map(|name| view! { <span>{name}</span> })
            .collect_view()}
    }
}

fn foreign_collect_view_is_not_leptos() {
    ["Ada"]
        .into_iter()
        .map(|name| view! { <span>{name}</span> })
        .foreign()
        .collect_view();
}

#[allow(rlib::leptos_manual_view_iteration)]
#[component]
fn Suppressed() -> impl IntoView {
    let (names, _) = signal(Vec::<String>::new());
    view! { {move || names.get().into_iter().map(|name| view! { <span>{name}</span> }).collect_view()} }
}

fn main() {}
