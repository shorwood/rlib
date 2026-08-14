#![allow(
    bon_undocumented_builder_members,
    dead_code,
    misordered_module_declarations,
    unknown_lints
)]

use bon::bon;

#[derive(bon::Builder)]
pub struct Request {
    host: String,
    port: u16,
    /// An absent label disables labeling.
    label: Option<String>,
    /// Number of retries; defaults to three.
    #[builder(default = 3)]
    retries: u32,
    #[builder(with = |default: u16| default)]
    secure_port: u16,
}

#[bon::builder]
pub fn upload(path: String, mut port: u16) {
    let _ = (path, port);
}

pub struct Client;

#[bon]
impl Client {
    #[builder]
    pub fn connect(host: String, timeout: u32) -> Self {
        let _ = (host, timeout);
        Self
    }
}

fn main() {}
