// compile-flags: --test

#![warn(unconsumed_generic_abstractions)]
#![allow(dead_code, method_like_free_functions)]

struct Production;
struct Substitute;

struct TestSubstitution<T>(T);
type ProductionUse = TestSubstitution<Production>;

#[cfg(test)]
type TestUse = TestSubstitution<Substitute>;

struct TestForwarding<T>(T);
type ConcreteUse = TestForwarding<Production>;

#[cfg(test)]
fn active_generic_consumer<T>(value: TestForwarding<T>) -> TestForwarding<T> {
    value
}
