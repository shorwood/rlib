// aux-build: external_macro.rs

#![warn(ambiguous_primitive_parameters)]
#![allow(dead_code, misordered_module_declarations)]

extern crate external_macro;

use external_macro::external_ambiguous_signature;

type UserNumber = u64;

struct Credentials {
    token: String,
}

fn schedule(user: UserNumber, project: u64, delay: u64) {}

fn authenticate(user: &str, organization: String, token: Box<str>) {}

fn validate_token(_token: &str, _backup: &str) -> Result<(), ()> {
    Ok(())
}

trait Router {
    fn route(user: u64, project: u64);
}

struct NetworkRouter;

impl Router for NetworkRouter {
    fn route(_user: u64, _project: u64) {}
}

struct Scheduler;

impl Scheduler {
    fn assign(&self, user: u64, project: u64) {}
}

fn point(x: f64, y: f64) {}

fn range(start: usize, end: usize) {}

fn compare(left: &u32, right: &u32) {}

fn replace(source: &str, pattern: &str, replacement: &str) {}

fn weak(a: u16, b: u16) {}

fn mixed(user: u64, project: u32) {}

// False-positive boundary: owned and borrowed scalars are not call-site interchangeable.
fn mixed_ownership(user: u64, project: &u64) {}

// False-positive boundary: shared and mutable borrows are not mutually interchangeable.
fn mixed_borrowing(user: &u64, project: &mut u64) {}

fn assign_references(user: &u64, project: &u64) {}

trait DefaultRouter {
    fn assign(user: u32, project: u32) {
        let _ = (user, project);
    }
}

macro_rules! local_ambiguous_signature {
    () => {
        fn local_assign(account: u32, project: u32) {}
    };
}

local_ambiguous_signature!();
external_ambiguous_signature!();

fn main() {}
