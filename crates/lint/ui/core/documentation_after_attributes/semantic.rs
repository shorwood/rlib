#![warn(rlib::documentation_after_attributes)]
#![allow(dead_code)]

#[allow(dead_code)]
/// Misplaced item documentation.
struct MisplacedItem {
    #[allow(dead_code)]
    /// Misplaced field documentation.
    field: usize,
}

enum MisplacedEnum {
    #[allow(dead_code)]
    /// Misplaced variant documentation.
    Variant {
        #[allow(dead_code)]
        #[doc = "Misplaced variant-field documentation."]
        field: usize,
    },
}

trait MisplacedTraitItem {
    #[allow(dead_code)]
    /// Misplaced associated-item documentation.
    fn method(&self);
}

struct Service;

impl Service {
    #[allow(dead_code)]
    /// Misplaced implementation-item documentation.
    fn method(&self) {}
}

/// Correctly leading documentation.
#[allow(dead_code)]
struct DocumentedFirst;

#[allow(rlib::documentation_after_attributes)]
#[allow(dead_code)]
/// Locally allowed misplaced documentation.
struct ExplicitlyAllowed;

macro_rules! generated_item {
    () => {
        #[allow(dead_code)]
        /// Expanded documentation is outside this lint's authored scope.
        struct Generated;
    };
}

generated_item!();

fn main() {}
