#![warn(rlib::ad_hoc_equality)]
#![allow(dead_code, rlib::misordered_module_declarations, rlib::missing_section_dividers)]

struct Slug(String);

fn slugs_equal(left: &Slug, right: &Slug) -> bool {
    left.0 == right.0
}

struct Account(u64);

impl Account {
    fn equals(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

struct Ambiguous(u64);

impl Ambiguous {
    fn equal(&self, other: &Self) -> bool {
        self.0 == other.0
    }

    fn same(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

struct Existing(u64);

impl PartialEq for Existing {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Existing {
    fn equal(&self, other: &Self) -> bool {
        self.0 % 10 == other.0 % 10
    }
}

struct Delegating(u64);

impl PartialEq for Delegating {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Delegating {
    fn equal(&self, other: &Self) -> bool {
        <Self as PartialEq>::eq(self, other)
    }
}

struct Contextual(u64);

impl Contextual {
    fn same_business_key(&self, other: &Self) -> bool {
        self.0 == other.0
    }

    fn constant_time_equal(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

struct Discarded(u64);

impl Discarded {
    // False-positive boundary: a discarded comparison does not determine the returned boolean.
    fn equal(&self, other: &Self) -> bool {
        let _discarded = self.0 == other.0;
        true
    }
}

struct Dormant(u64);

impl Dormant {
    // False-positive boundary: an uncalled closure does not determine the returned boolean.
    fn equal(&self, other: &Self) -> bool {
        let _comparison = || self.0 == other.0;
        false
    }
}

struct Conventional(u64);

impl Conventional {
    // False-negative boundary: predicate-style equality is still an unqualified relation.
    fn is_equal(&self, other: &Self) -> bool {
        self.0 == other.0
    }

    // False-negative boundary: canonical inequality is also owned by `PartialEq`.
    fn not_equal(&self, other: &Self) -> bool {
        self.0 != other.0
    }
}

struct Early(u64);

impl Early {
    // False-negative boundary: an explicit return can carry the canonical relation.
    fn equal(&self, other: &Self) -> bool {
        if self.0 == 0 {
            return self.0 == other.0;
        }
        false
    }
}

struct Aliased(u64);

impl Aliased {
    // False-negative boundary: a returned local preserves its comparison provenance.
    fn equal(&self, other: &Self) -> bool {
        let equal = self.0 == other.0;
        equal
    }
}

fn main() {}
