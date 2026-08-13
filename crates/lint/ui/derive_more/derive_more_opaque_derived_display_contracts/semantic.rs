#![allow(dead_code, misordered_module_declarations, unknown_lints)]

use std::collections::HashMap;

#[derive(derive_more::Display)]
#[display("{kind}:{value}")]
struct CacheKey {
    kind: u8,
    value: String,
}

fn cache_record(cache: &mut HashMap<String, u32>, key: CacheKey) {
    cache.insert(key.to_string(), 1);
}

#[derive(derive_more::Display)]
struct Label(String);

impl Label {
    fn render(self) {
        println!("{self}");
    }
}

fn main() {}
