// aux-build: external_macro.rs
// edition:2024

#![feature(register_tool)]
#![allow(dead_code, improper_ctypes_definitions, unused_variables, enforce_inherent_impl_item_order, enforce_module_declaration_order, enforce_type_dependency_order)]
#![register_tool(rlib_lint)]

extern crate external_macro;

use std::collections::VecDeque;

use external_macro::{external_struct, external_vector_function};

#[path = "support/remote.inc"]
mod remote;

// Every direct ownership form needs the receiver that preserves its original contract.
struct Item;

#[allow(non_snake_case)]
fn ItemList() {}

fn owned(items: Vec<Item>) {}
fn mutable_owned(mut items: Vec<Item>) {}
fn shared_vector(items: &Vec<Item>) {}
fn mutable_vector(items: &mut Vec<Item>) {}
fn shared_slice(items: &[Item]) {}
fn mutable_slice(items: &mut [Item]) {}

// Transparent aliases do not hide the collection or its element from the rule.
type ItemAlias = Item;
type Items = Vec<ItemAlias>;
type ItemSlice<'a> = &'a [ItemAlias];

fn aliased_vector(items: Items) {}
fn aliased_slice(items: ItemSlice<'_>) {}

// Generic element structs use a correspondingly generic wrapper recipe.
struct Generic<T>(T);

fn generic<T>(items: Vec<Generic<T>>) {}

// A compatible canonical wrapper should be reused instead of recreated.
struct Existing;
struct ExistingList {
    items: Vec<Existing>,
    label: String,
}

impl ExistingList {}

fn use_existing(items: &[Existing]) {}

// An occupied canonical name needs a manual naming decision.
struct Conflict;
struct ConflictList {
    other: usize,
}

fn conflicting_wrapper(items: Vec<Conflict>) {}

// Local macro output belongs to this crate and remains enforceable.
macro_rules! local_vector_function {
    () => {
        fn from_local_macro(items: Vec<Item>) {
            let _ = items;
        }
    };
}

local_vector_function!();

// External macro output is not editable here, whether it creates the function or element struct.
external_vector_function!(Item);
external_struct!();

fn external_element(items: Vec<ExternalItem>) {}

// Only a direct collection in the first parameter is governed by this rule.
fn second_parameter(count: usize, items: Vec<Item>) {}
fn boxed_elements(items: Vec<Box<Item>>) {}
fn referenced_elements(items: Vec<&Item>) {}
fn fixed_array(items: [Item; 2]) {}
fn deque(items: VecDeque<Item>) {}
fn unknown_element<T>(items: Vec<T>) {}

enum NotAStruct {
    Value,
}

union NotAStructEither {
    value: u8,
}

fn enum_elements(items: Vec<NotAStruct>) {}
fn union_elements(items: Vec<NotAStructEither>) {}
extern "C" fn foreign_abi(items: Vec<Item>) {}

// Cross-module functions still belong on a wrapper beside the element struct.
mod elsewhere {
    pub(super) struct Other;

    pub(super) struct Reusable;
    pub(super) struct ReusableList {
        items: Vec<Reusable>,
    }

    impl ReusableList {}

    pub(super) struct Occupied;
    pub(super) enum OccupiedList {
        Value,
    }
}

// A same-named wrapper in the function's module does not own `elsewhere::Other`.
struct OtherList;

fn different_module(items: Vec<elsewhere::Other>) {}
fn reuse_different_module(items: &[elsewhere::Reusable]) {}
fn conflict_different_module(items: Vec<elsewhere::Occupied>) {}
fn different_file(items: &[remote::Remote]) {}

mod child {
    fn different_module(items: Vec<super::Item>) {}
}

fn nested_function_is_out_of_scope() {
    struct Local;
    fn nested(items: Vec<Local>) {}
}

// Associated functions are already organized under a type and are not free functions.
struct Service;

impl Service {
    fn associated(items: Vec<Item>) {}
}

#[allow(enforce_implementable_methods_on_vectors)]
fn explicitly_allowed(items: Vec<Item>) {}

fn main() {}
