#![allow(dead_code, misordered_module_declarations, unknown_lints)]

use bon::bon;

#[bon::builder(finish_fn = execute)]
fn publish_report(#[builder(name = data)] destination: String) {
    let _ = destination;
}

#[bon::builder(finish_fn = publish)]
fn publish_document(destination: String) {
    let _ = destination;
}

struct Publisher;

#[bon]
impl Publisher {
    #[builder(finish_fn = process)]
    fn publish_article(#[builder(name = value)] destination: String) -> Self {
        let _ = destination;
        Self
    }
}

#[derive(bon::Builder)]
#[builder(finish_fn = execute)]
struct Delivery {
    #[builder(name = data)]
    destination: String,
}

fn main() {}
