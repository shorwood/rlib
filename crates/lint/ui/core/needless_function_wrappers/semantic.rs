// edition:2024

#![feature(register_tool)]
#![allow(
    dead_code,
    improper_ctypes_definitions,
    unconditional_recursion,
    misordered_inherent_impl_items,
    misordered_module_declarations,
    non_adjacent_struct_impls,
    misordered_type_declarations
)]
#![register_tool(rlib_lint)]

#[path = "support/remote.inc"]
mod remote;

fn target(value: u32) -> u32 {
    value
}

fn no_arguments_target() -> u32 {
    1
}

fn generic_target<T>(value: T) -> T {
    value
}

fn pair_target(left: u32, right: u32) -> u32 {
    left + right
}

fn string_target(value: &str) -> usize {
    value.len()
}

async fn async_target(value: u32) -> u32 {
    value
}

fn future_target(value: u32) -> impl Future<Output = u32> {
    async move { value }
}

fn direct(value: u32) -> u32 {
    target(value)
}

fn explicit_return(value: u32) -> u32 {
    return target(value);
}

fn no_arguments() -> u32 {
    no_arguments_target()
}

fn generic<T>(value: T) -> T {
    generic_target(value)
}

fn cross_file(value: u32) -> u32 {
    remote::remote_target(value)
}

/// This documentation belongs on `target`.
pub fn documented_facade(value: u32) -> u32 {
    target(value)
}

pub fn public_facade(value: u32) -> u32 {
    target(value)
}

/// Documentation alone does not justify an indirection.
fn private_documented(value: u32) -> u32 {
    target(value)
}

fn chain_middle(value: u32) -> u32 {
    target(value)
}

fn chain_facade(value: u32) -> u32 {
    chain_middle(value)
}

async fn async_direct(value: u32) -> u32 {
    async_target(value).await
}

struct Service;

impl Service {
    fn target(&self, value: u32) -> u32 {
        value
    }

    fn method(&self, value: u32) -> u32 {
        self.target(value)
    }

    fn associated_target(value: u32) -> u32 {
        value
    }

    fn associated(value: u32) -> u32 {
        Self::associated_target(value)
    }

    async fn async_target(&self, value: u32) -> u32 {
        value
    }

    async fn async_method(&self, value: u32) -> u32 {
        self.async_target(value).await
    }
}

impl Service {
    fn method_from_separate_impl(&self, value: u32) -> u32 {
        self.target(value)
    }
}

struct ServiceWrapper(Service);

impl std::ops::Deref for ServiceWrapper {
    type Target = Service;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ServiceWrapper {
    fn receiver_adapter(&self, value: u32) -> u32 {
        self.target(value)
    }
}

// Argument or result adaptation gives these functions meaning.
fn reordered(left: u32, right: u32) -> u32 {
    pair_target(right, left)
}

fn duplicated(left: u32, _right: u32) -> u32 {
    pair_target(left, left)
}

fn transformed(value: u32) -> u32 {
    target(value + 1)
}

fn wrapped_result(value: u32) -> Option<u32> {
    Some(target(value))
}

fn coerced(value: &String) -> usize {
    string_target(value)
}

fn extra_statement(value: u32) -> u32 {
    let value = value;
    target(value)
}

async fn async_extra_statement(value: u32) -> u32 {
    let value = value;
    async_target(value).await
}

async fn awaits_non_async_function(value: u32) -> u32 {
    future_target(value).await
}

// These boundaries are deliberately outside the rule.
trait Behavior {
    fn run(&self, value: u32) -> u32;
}

impl Behavior for Service {
    fn run(&self, value: u32) -> u32 {
        self.target(value)
    }
}

unsafe fn unsafe_wrapper(value: u32) -> u32 {
    target(value)
}

extern "C" fn abi_wrapper(value: u32) -> u32 {
    target(value)
}

fn external_target(value: u32) -> u32 {
    std::hint::black_box(value)
}

unsafe fn unsafe_target(value: u32) -> u32 {
    value
}

fn safe_boundary(value: u32) -> u32 {
    // The wrapper owns the proof that makes this unsafe call valid.
    unsafe { unsafe_target(value) }
}

fn function_pointer(target: fn(u32) -> u32, value: u32) -> u32 {
    target(value)
}

#[inline]
fn attributed(value: u32) -> u32 {
    target(value)
}

fn recursive(value: u32) -> u32 {
    recursive(value)
}

macro_rules! generated_wrapper {
    () => {
        fn generated(value: u32) -> u32 {
            target(value)
        }
    };
}

generated_wrapper!();

fn main() {}
