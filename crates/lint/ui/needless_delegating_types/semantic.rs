#![warn(needless_delegating_types)]
#![allow(
    dead_code,
    method_like_free_functions,
    misordered_module_declarations,
    non_adjacent_struct_impls
)]

struct Inner;

impl Inner {
    fn read(&self, value: u8) -> u8 {
        value
    }

    fn write(&self, value: u16) -> u16 {
        value
    }
}

/// Documentation alone does not establish wrapper ownership.
struct Wrapper(Inner);

impl Wrapper {
    fn read(&self, value: u8) -> u8 {
        self.0.read(value)
    }

    fn write(&self, value: u16) -> u16 {
        self.0.write(value)
    }
}

pub struct RecordWrapper {
    inner: Inner,
}

impl RecordWrapper {
    fn new(inner: Inner) -> Self {
        Self { inner }
    }

    fn inner(&self) -> &Inner {
        &self.inner
    }

    fn read(&self, value: u8) -> u8 {
        self.inner.read(value)
    }

    fn write(&self, value: u16) -> u16 {
        self.inner.write(value)
    }
}

struct OneMethod(Inner);

impl OneMethod {
    fn read(&self, value: u8) -> u8 {
        self.0.read(value)
    }
}

struct Substantive(Inner);

impl Substantive {
    fn new() -> Self {
        Self(Inner)
    }

    fn read(&self, value: u8) -> u8 {
        self.0.read(value)
    }

    fn write(&self, value: u16) -> u16 {
        self.0.write(value)
    }
}

struct Contract(Inner);

impl Contract {
    fn read(&self, value: u8) -> u8 {
        self.0.read(value)
    }

    fn write(&self, value: u16) -> u16 {
        self.0.write(value)
    }
}

impl std::fmt::Debug for Contract {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Contract")
    }
}

struct Generic<T>(T);

fn main() {
    let wrapper = Wrapper(make_inner());
    let _ = wrapper.read(1);
    let _ = wrapper.write(2);
    let record = RecordWrapper::new(Inner);
    let _ = record.inner();
    let _ = record.read(1);
    let _ = record.write(2);
    let one = OneMethod(Inner);
    let _ = one.read(1);
    let substantive = Substantive::new();
    let _ = substantive.read(1);
    let _ = substantive.write(2);
    let contract = Contract(Inner);
    let _ = contract.read(1);
    let _ = contract.write(2);
    let _ = Generic(Inner);
    let representation = Representation(Inner);
    let _ = representation.read(1);
    let _ = representation.write(2);
}

fn make_inner() -> Inner {
    Inner
}

#[repr(transparent)]
struct Representation(Inner);

impl Representation {
    fn read(&self, value: u8) -> u8 {
        self.0.read(value)
    }

    fn write(&self, value: u16) -> u16 {
        self.0.write(value)
    }
}

macro_rules! generated_wrapper {
    () => {
        struct GeneratedWrapper(Inner);

        impl GeneratedWrapper {
            fn read(&self, value: u8) -> u8 {
                self.0.read(value)
            }

            fn write(&self, value: u16) -> u16 {
                self.0.write(value)
            }
        }

        fn generated_use() {
            let value = GeneratedWrapper(Inner);
            let _ = value.read(1);
            let _ = value.write(2);
        }
    };
}

generated_wrapper!();
