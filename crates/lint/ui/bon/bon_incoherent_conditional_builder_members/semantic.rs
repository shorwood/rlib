#![allow(
    dead_code,
    misordered_module_declarations,
    unexpected_cfgs,
    unknown_lints
)]

#[bon::builder]
fn connect(#[cfg_attr(feature = "lenient", builder(default))] timeout: u64) {
    let _ = timeout;
}

#[bon::builder]
fn stable(#[builder(default)] timeout: u64) {
    let _ = timeout;
}

#[derive(bon::Builder)]
struct PlatformCapability {
    #[cfg(feature = "gpu")]
    device: String,
}

fn main() {}
