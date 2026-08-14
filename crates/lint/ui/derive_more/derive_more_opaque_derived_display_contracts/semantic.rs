#![allow(
    unknown_lints,
    dead_code,
    method_like_free_functions,
    misordered_module_declarations
)]

use std::collections::{BTreeMap, HashMap};

#[derive(derive_more::Display)]
#[display("{kind}:{value}")]
struct CacheKey {
    kind: u8,
    value: String,
}

fn cache_record(cache: &mut HashMap<String, u32>, key: CacheKey) {
    cache.insert(key.to_string(), 1);
}

fn ordered_cache_record(cache: &mut BTreeMap<String, u32>, key: &CacheKey) {
    cache.insert(key.to_string(), 1);
}

macro_rules! generated_cache_record {
    ($cache:expr, $key:expr) => {
        $cache.insert($key.to_string(), 1)
    };
}

fn generated_use(cache: &mut HashMap<String, u32>, key: CacheKey) {
    generated_cache_record!(cache, key);
}

struct LocalMap;

impl LocalMap {
    fn insert(&mut self, _key: String, _value: u32) {}
}

fn unrelated_insert(cache: &mut LocalMap, key: CacheKey) {
    cache.insert(key.to_string(), 1);
}

mod custom_to_string_case {
    #![no_implicit_prelude]

    extern crate derive_more;

    use ::std::collections::HashMap;
    use ::std::string::String;

    trait ToString {
        fn to_string(&self) -> String;
    }

    #[derive(::derive_more::Display)]
    struct Key(u8);

    impl ToString for Key {
        fn to_string(&self) -> String {
            String::new()
        }
    }

    fn custom_to_string(cache: &mut HashMap<String, u32>, key: Key) {
        cache.insert(key.to_string(), 1);
    }
}

#[derive(derive_more::Display)]
struct Label(String);

impl Label {
    fn render(self) {
        println!("{self}");
    }
}

struct ManualDisplay;

impl std::fmt::Display for ManualDisplay {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("manual")
    }
}

fn manual_display(cache: &mut HashMap<String, u32>, key: ManualDisplay) {
    cache.insert(key.to_string(), 1);
}

fn main() {}
