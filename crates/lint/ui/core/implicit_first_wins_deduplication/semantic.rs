#![warn(implicit_first_wins_deduplication)]
#![allow(dead_code, misordered_module_declarations)]

use std::collections::{HashMap, HashSet};

fn direct(values: &[u32]) {
    let mut seen = HashSet::new();
    let _ = values.iter().filter(|value| seen.insert(**value));
}

fn block(values: &[u32]) {
    let mut seen = HashSet::new();
    let _ = values.iter().filter(|value| {
        seen.insert(**value)
    });
}

fn ordinary_predicate(values: &[u32]) {
    let seen = HashSet::from([1]);
    let _ = values.iter().filter(|value| seen.contains(value));
}

struct CustomFilter;

impl CustomFilter {
    fn filter(self, predicate: impl FnOnce(u32) -> bool) {
        let _ = predicate(1);
    }
}

fn same_named_custom_method() {
    let mut seen = HashSet::new();
    CustomFilter.filter(|value| seen.insert(value));
}

fn explicit_collision_policy(values: &[u32]) {
    let mut unique = HashMap::new();
    for value in values {
        unique.entry(*value).or_insert(value);
    }
}

fn main() {}
