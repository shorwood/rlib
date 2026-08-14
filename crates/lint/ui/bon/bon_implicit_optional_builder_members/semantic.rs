#![allow(dead_code, misordered_module_declarations, unknown_lints)]

use std::time::Duration;

use bon::bon;

#[bon::builder]
fn request(timeout: Option<Duration>, label: Option<String>) {
    let _ = (timeout, label);
}

#[bon::builder]
fn deliberate(#[builder(required)] timeout: Option<Duration>) {
    let _ = timeout;
}

#[bon::builder(on(_, required))]
fn required_by_policy(limit: Option<u32>) {
    let _ = limit;
}

#[bon::builder]
fn documented(
    /// Omitting the destination uses the service's configured default.
    destination: Option<String>,
) {
    let _ = destination;
}

struct Client;

#[bon]
impl Client {
    #[builder]
    fn connect(timeout: Option<Duration>) -> Self {
        let _ = timeout;
        Self
    }
}

fn main() {}
