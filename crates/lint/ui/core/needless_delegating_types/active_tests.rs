// compile-flags: --test

#![warn(rlib::needless_delegating_types)]
#![allow(dead_code, rlib::method_like_free_functions, rlib::non_adjacent_struct_impls)]

struct Inner;

impl Inner {
    fn read(&self, value: u8) -> u8 {
        value
    }

    fn write(&self, value: u16) -> u16 {
        value
    }
}

struct TestBehavior(Inner);

impl TestBehavior {
    fn read(&self, value: u8) -> u8 {
        self.0.read(value)
    }

    fn write(&self, value: u16) -> u16 {
        self.0.write(value)
    }

    #[cfg(test)]
    fn reset_for_test(&mut self) {
        self.0 = Inner;
    }
}

fn construct() -> TestBehavior {
    TestBehavior(Inner)
}
