#![allow(dead_code, misordered_module_declarations, unknown_lints)]

#[bon::builder(finish_fn = execute)]
fn publish_report(#[builder(name = data)] destination: String) {
    let _ = destination;
}

#[bon::builder(finish_fn = publish)]
fn publish_document(destination: String) {
    let _ = destination;
}

fn main() {}
