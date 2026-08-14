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

struct AliasedTotalFloat(f64);

impl AliasedTotalFloat {
    // False-negative boundary: returned aliases preserve the total-ordering operation.
    fn compare(&self, other: &Self) -> Ordering {
        let ordering = self.0.total_cmp(&other.0);
        ordering
    }
}

struct EarlyOrdering(u64);

impl EarlyOrdering {
    fn compare(&self, other: &Self) -> Ordering {
        return self.0.cmp(&other.0);
    }
}

struct DiscardedOrdering(u64);

impl DiscardedOrdering {
    // False-positive boundary: a discarded comparison does not define the returned ordering.
    fn compare(&self, other: &Self) -> Ordering {
        let _ = self.0.cmp(&other.0);
        Ordering::Equal
    }
}

struct DormantOrdering(u64);

impl DormantOrdering {
    // False-positive boundary: a closure-local comparison does not define this method's result.
    fn compare(&self, other: &Self) -> Ordering {
        let _later = || self.0.cmp(&other.0);
        Ordering::Equal
    }
}

struct IncidentalTotalFloat(f64);

impl IncidentalTotalFloat {
    // False-positive boundary: discarded totalization cannot legitimize a partial relation.
    fn compare(&self, other: &Self) -> Ordering {
        let _ = self.0.total_cmp(&other.0);
        self.0.partial_cmp(&other.0).unwrap_or(Ordering::Equal)
    }
}

#[derive(Eq, Ord, PartialEq, PartialOrd)]
struct DelegatingOwned(u64);

impl DelegatingOwned {
    // Delegation to the occupied standard contract is a compatibility alias.
    fn compare(&self, other: &Self) -> Ordering {
        self.cmp(other)
    }
}

#[derive(Eq, Ord, PartialEq, PartialOrd)]
struct CompetingOwned(u64);

impl CompetingOwned {
    // A detached field comparator can drift from the type's occupied `Ord` contract.
    fn compare(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

fn main() {}
