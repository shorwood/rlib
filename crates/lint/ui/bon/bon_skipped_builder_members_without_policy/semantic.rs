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

fn main() {}
