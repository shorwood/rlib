// compile-flags: --test

#![warn(rlib::single_implementation_traits)]
#![allow(dead_code)]

trait TestSubstitution {
    fn substitute(&self);
}

struct Production;

impl TestSubstitution for Production {
    fn substitute(&self) {}
}

#[cfg(test)]
struct Substitute;

#[cfg(test)]
impl TestSubstitution for Substitute {
    fn substitute(&self) {}
}

trait TestConsumer {
    fn consume(&self);
}

struct TestConsumerImpl;

impl TestConsumer for TestConsumerImpl {
    fn consume(&self) {}
}

#[cfg(test)]
fn active_polymorphic_consumer<T: TestConsumer>(value: &T) {
    value.consume();
}
