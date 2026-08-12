#![warn(ad_hoc_ordering)]
#![allow(dead_code, misordered_module_declarations, missing_section_dividers)]

use std::cmp::Ordering;

struct Version(u64);

impl Version {
    fn compare(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

struct Score(u64);

fn partial_compare_score(left: &Score, right: &Score) -> Option<Ordering> {
    Some(left.0.cmp(&right.0))
}

struct Ambiguous(u64);

impl Ambiguous {
    fn compare(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }

    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

struct Contextual(u64);

impl Contextual {
    fn compare_for_display(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

struct FloatValue(f64);

impl FloatValue {
    fn compare(&self, other: &Self) -> Ordering {
        self.0.partial_cmp(&other.0).unwrap_or(Ordering::Equal)
    }
}

struct TotalFloat(f64);

impl TotalFloat {
    fn compare(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0)
    }
}

fn main() {}
