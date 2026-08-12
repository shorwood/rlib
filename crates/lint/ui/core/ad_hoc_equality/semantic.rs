#![warn(ad_hoc_equality)]
#![allow(dead_code, misordered_module_declarations, missing_section_dividers)]

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

fn main() {}
