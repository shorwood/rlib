#![warn(needlessly_nested_control_flow)]
#![allow(dead_code, misordered_module_declarations)]

fn work() {}

fn diverging_then(valid: bool) {
    if !valid {
        return;
    } else {
        work();
    }
}

fn diverging_else(valid: bool) {
    if valid {
        work();
    } else {
        panic!("invalid");
    }
}

fn fail() -> ! {
    panic!("failed")
}

fn indirect_divergence(valid: bool) {
    if !valid {
        fail();
    } else {
        work();
    }
}

fn trailing_condition(valid: bool) {
    if valid {
        work();
    }
}

fn trailing_pattern(value: Option<u8>) {
    if let Some(_value) = value {
        work();
    }
}

fn loop_guard(values: &[bool]) {
    for value in values {
        if *value {
            work();
        }
    }
}

fn already_flat(valid: bool) {
    if !valid {
        return;
    }
    work();
}

macro_rules! generated_condition {
    () => {
        fn generated_condition(valid: bool) {
            if valid {
                work();
            }
        }
    };
}

generated_condition!();

#[allow(needlessly_nested_control_flow)]
fn explicitly_allowed(valid: bool) {
    if valid {
        work();
    }
}

fn main() {}

// A final error handler already exits and does not hide useful work behind a condition.
fn terminal_error(result: Result<(), ()>) {
    if let Err(()) = result {
        panic!("failed");
    }
}

fn terminal_continue(values: &[bool]) {
    for value in values {
        if !*value {
            continue;
        }
    }
}
