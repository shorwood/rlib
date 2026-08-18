// compile-flags: --test

#![warn(rlib::undocumented_items)]

fn helper() {}

#[test]
fn test_case_names_the_scenario() {
    helper();
}
