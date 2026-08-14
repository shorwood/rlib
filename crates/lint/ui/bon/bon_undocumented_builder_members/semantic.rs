#![allow(dead_code, misordered_module_declarations, unknown_lints)]

use bon::bon;

#[bon::builder]
pub fn connect(#[builder(default = 30)] timeout_seconds: u64, host: String) {
    let _ = (timeout_seconds, host);
}

#[bon::builder]
pub fn documented(
    /// An absent label disables display labeling.
    label: Option<String>,
) {
    let _ = label;
}

#[bon::builder]
fn private_api(#[builder(into)] host: String) {
    let _ = host;
}

#[derive(bon::Builder)]
pub struct Request {
    #[builder(into)]
    destination: String,
    payload: Vec<u8>,
    #[doc = ""]
    optional_label: Option<String>,
    #[builder(with = |doc: String| doc)]
    alias: String,
    #[builder(default, setters(doc {
        /// Defaults to the domain's standard retry policy.
    }))]
    retries: u32,
}

pub struct Client;

#[bon]
impl Client {
    #[builder]
    pub fn connect(#[builder(default = 30)] timeout_seconds: u64) -> Self {
        let _ = timeout_seconds;
        Self
    }
}

mod internal {
    #[bon::builder]
    pub fn hidden(#[builder(default)] retries: u32) {
        let _ = retries;
    }

    #[derive(bon::Builder)]
    pub struct HiddenState {
        #[builder(into)]
        value: String,
    }
}

fn main() {}
