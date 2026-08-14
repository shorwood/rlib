#![allow(dead_code, misordered_module_declarations, unknown_lints)]

use std::marker::PhantomData;

#[derive(bon::Builder)]
struct Session {
    #[builder(skip)]
    revision: u64,
    name: String,
}

#[derive(bon::Builder)]
struct Explicit {
    #[builder(skip = 1)]
    revision: u64,
}

#[derive(bon::Builder)]
struct Documented {
    /// New values deliberately begin at revision zero.
    #[builder(skip)]
    revision: u64,
}

#[derive(bon::Builder)]
struct Marker<T> {
    #[builder(skip)]
    marker: PhantomData<T>,
}

#[derive(bon::Builder)]
struct CommentedSkip {
    #[builder(/* still bare */ skip)]
    revision: u64,
}

#[derive(bon::Builder)]
struct EmptyDocumentation {
    #[doc = ""]
    #[builder(skip)]
    revision: u64,
}

struct NotPhantomData<T>(PhantomData<T>);

impl<T> Default for NotPhantomData<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

#[derive(bon::Builder)]
struct NamedLikeMarker<T> {
    #[builder(skip)]
    marker: NotPhantomData<T>,
}

#[derive(bon::Builder)]
struct QualifiedMarker<T> {
    #[builder(skip)]
    marker: std::marker::PhantomData<T>,
}

fn main() {}
