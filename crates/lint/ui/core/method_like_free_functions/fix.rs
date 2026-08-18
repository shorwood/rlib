// run-rustfix
// rustfix-only-machine-applicable

#![feature(register_tool)]
#![allow(dead_code, rlib::misordered_inherent_impl_items, rlib::misordered_module_declarations, rlib::misordered_type_declarations)]
#![register_tool(rlib_lint)]

struct Counter {
    value: u8,
}

struct Snapshot {
    counter: Counter,
}

struct Generic<T>(T);

// Moving a value receiver must also rename its uses, including shorthand fields.
fn bump(mut counter: Counter, amount: u8) -> Counter {
    counter.value += amount;
    let snapshot = Snapshot { counter };
    snapshot.counter
}

// Borrowing, explicit lifetimes, and mutable borrowing retain their receiver shape.
fn inspect<'a>(counter: &'a Counter) -> u8 {
    counter.value
}

fn reset(counter: &mut Counter) {
    counter.value = 0;
}

// A simple type parameter moves to the impl when the receiver uses it.
fn unwrap<T>(item: Generic<T>) -> T {
    item.0
}

// Calls and function values use method UFCS after the free function moves.
fn use_inspect(counter: &Counter) -> u8 {
    let callback: fn(&Counter) -> u8 = inspect;
    inspect(counter) + callback(counter)
}

fn main() {}
