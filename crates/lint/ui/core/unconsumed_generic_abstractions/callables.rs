#![warn(rlib::unconsumed_generic_abstractions)]
#![allow(
    dead_code,
    rlib::method_like_free_functions,
    rlib::misordered_module_declarations,
    rlib::non_adjacent_struct_impls,
    private_interfaces,
    unused_variables
)]

/// Documentation does not establish a second substitution boundary.
fn inferred_and_explicit<T>(value: T) -> T {
    value
}

fn mixed<T, U>(first: T, second: U) -> (T, U) {
    (first, second)
}

pub fn closed_package<T>(value: T) -> T {
    value
}

struct Service;

impl Service {
    fn associated<T>(value: T) -> T {
        value
    }

    fn method<T>(&self, value: T) -> T {
        value
    }
}

fn varying<T>(value: T) -> T {
    value
}

fn generically_forwarded<T>(value: T) -> T {
    value
}

unsafe fn forwarding_boundary<U>(value: U) -> U {
    generically_forwarded(value)
}

fn escaped<T>(value: T) -> T {
    value
}

fn closure_boundary<T, F: FnOnce(T) -> T>(value: T, callback: F) -> T {
    callback(value)
}

type ConcreteAlias = u8;

fn alias_boundary<T>(value: T) -> T {
    value
}

fn function_pointer_boundary<T>(value: T) -> T {
    value
}

fn callback(value: u8) -> u8 {
    value
}

trait Contract {
    fn trait_method<T>(&self, value: T) -> T;
}

impl Contract for Service {
    fn trait_method<T>(&self, value: T) -> T {
        value
    }
}

struct GenericHost<T>(T);

impl<T> GenericHost<T> {
    fn impl_parameter(&self, value: T) -> &T {
        let _ = value;
        &self.0
    }
}

fn never_called<T>(value: T) -> T {
    value
}

fn main() {
    let _ = inferred_and_explicit(1_u8);
    let _ = inferred_and_explicit::<u8>(2);

    let _ = mixed(1_u8, 2_u16);
    let _ = mixed(3_u8, 4_u32);

    let _ = closed_package(5_u16);

    let service = Service;
    let _ = Service::associated(6_u32);
    let _ = service.method(7_u64);

    let _ = varying(8_u8);
    let _ = varying(9_u16);

    let _ = escaped::<u8>;
    let _ = closure_boundary(10_u8, |value| value);
    let _ = alias_boundary::<ConcreteAlias>(10);
    let _ = function_pointer_boundary(callback as fn(u8) -> u8);
    let _ = service.trait_method(11_u8);
    let host = GenericHost(12_u8);
    let _ = host.impl_parameter(13_u8);
}
