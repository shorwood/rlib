// aux-build: external_macro.rs
// edition:2024

#![feature(register_tool)]
#![allow(dead_code, improper_ctypes_definitions, unused_variables, misordered_inherent_impl_items, misordered_module_declarations, misordered_type_declarations)]
#![register_tool(rlib_lint)]

extern crate external_macro;

use external_macro::{external_function, external_struct};

// Direct values and references preserve their ownership when they become receivers.
struct Item<T> {
    value: T,
}

type ItemAlias<T> = Item<T>;

type ItemRef<'a> = &'a Item<u8>;

fn consume(item: Item<u8>) {}

fn inspect(item: &Item<u8>) {}

fn mutate(item: &mut ItemAlias<u8>) {}

fn inspect_alias(item: ItemRef<'_>) {}

// Every Rust-ABI function form can retain its modifier as an inherent method.
async fn inspect_async(item: &Item<u8>) {}

const fn inspect_const(item: &Item<u8>) {}

unsafe fn inspect_unsafe(item: &Item<u8>) {}

extern "Rust" fn inspect_rust_abi(item: &Item<u8>) {}

// All struct shapes are eligible.
struct Tuple(u8);

trait Behavior {
    fn trait_method(&self);
}

struct Unit;

impl Unit {
    fn already_in_impl(value: &Self) {}
}

impl Behavior for Unit {
    fn trait_method(&self) {}
}

fn inspect_tuple(value: &Tuple) {}

fn inspect_unit(value: &Unit) {}

// A delegating wrapper is still a free function, even when a same-named method exists.
struct Wrapper;

impl Wrapper {
    fn execute(&self) {}
}

fn execute(value: &Wrapper) {
    value.execute();
}

// Local macro output belongs to this crate and should be reported.
macro_rules! local_function {
    () => {
        fn from_local_macro(value: &Unit) {
            let _ = value;
        }
    };
}

local_function!();

// External macro output is not editable here, whether it creates the function or the struct.
external_function!(Unit);
external_struct!();

fn for_external_struct(value: &ExternalStruct) {}

// False-positive boundary: framework-generated component glue is not an authored ownership API.
fn __component_generated(value: &Unit) {}

// These forms cannot map directly to a receiver and remain valid free functions.
fn second_parameter(count: usize, item: &Item<u8>) {}

fn boxed(item: Box<Item<u8>>) {}

fn raw(item: *const Item<u8>) {}

fn double_reference(item: &&Item<u8>) {}

extern "C" fn foreign_abi(item: &Item<u8>) {}

enum NotAStruct {
    Value,
}

union NotAStructEither {
    value: u8,
}

fn enum_parameter(value: &NotAStruct) {}

fn union_parameter(value: &NotAStructEither) {}

// Importing a struct does not make its defining module the function's module.
mod elsewhere {
    pub(super) struct Other;
}

fn different_module(value: &elsewhere::Other) {}

mod child {
    fn different_module(value: &super::Item<u8>) {}
}

// Constructors, inherent items, trait items, and explicit suppression are not findings.
fn make_item() -> Item<u8> {
    Item { value: 0 }
}

#[allow(method_like_free_functions)]
fn explicitly_allowed(value: &Unit) {}

fn main() {}
