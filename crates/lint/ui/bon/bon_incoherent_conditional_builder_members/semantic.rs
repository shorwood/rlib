#![allow(
    dead_code,
    rlib::bon_implicit_optional_builder_members,
    rlib::bon_needless_builders_for_small_apis,
    rlib::misordered_module_declarations,
    unexpected_cfgs,
    unknown_lints
)]

use bon::bon;

#[bon::builder]
fn connect(#[cfg_attr(feature = "lenient", builder(default))] timeout: u64) {
    let _ = timeout;
}

#[bon::builder]
fn stable(#[builder(default)] timeout: u64) {
    let _ = timeout;
}

#[bon::builder]
fn policy_word_only_in_predicate(#[cfg_attr(feature = "required", builder(into))] timeout: u64) {
    let _ = timeout;
}

struct Connector;

#[bon]
impl Connector {
    #[builder]
    fn open(#[cfg_attr(feature = "lenient", builder(required))] timeout: Option<u64>) -> Self {
        let _ = timeout;
        Self
    }
}

#[derive(bon::Builder)]
struct PlatformCapability {
    #[cfg(feature = "gpu")]
    device: String,
}

#[derive(bon::Builder)]
struct ConditionalField {
    #[cfg_attr(feature = "lenient", builder(default))]
    timeout: u64,
}

fn main() {}
