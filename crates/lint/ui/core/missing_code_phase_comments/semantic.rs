#![warn(rlib::missing_code_phase_comments)]
#![allow(dead_code, rlib::misordered_module_declarations)]

fn missing() {
    let _one = 1;
    let _two = 2;
    let _three = 3;
    let _four = 4;
    let _five = 5;
    let _six = 6;
    let _seven = 7;
    let _eight = 8;
    let _nine = 9;
    let _ten = 10;
    let _eleven = 11;
    let _twelve = 12;
    let _thirteen = 13;
    let _fourteen = 14;
    let _fifteen = 15;
    let _sixteen = 16;
}

async fn missing_async() {
    let _one = 1;
    let _two = 2;
    let _three = 3;
    let _four = 4;
    let _five = 5;
    let _six = 6;
    let _seven = 7;
    let _eight = 8;
}

fn exact_limit() {
    let _one = 1;
    let _two = 2;
    let _three = 3;
    let _four = 4;
    let _five = 5;
    let _six = 6;
    let _seven = 7;
    let _eight = 8;
    let _nine = 9;
    let _ten = 10;
    let _eleven = 11;
    let _twelve = 12;
    let _thirteen = 13;
    let _fourteen = 14;
    let _fifteen = 15;
}

#[allow(rlib::missing_code_phase_comments)]
fn explicitly_allowed() {
    let _one = 1;
    let _two = 2;
    let _three = 3;
    let _four = 4;
    let _five = 5;
    let _six = 6;
    let _seven = 7;
    let _eight = 8;
}

fn named_phases() {
    // Prepare the first group of values.
    let _one = 1;
    let _two = 2;
    let _three = 3;
    let _four = 4;

    // Complete the remaining values.
    let _five = 5;
    let _six = 6;
    let _seven = 7;
    let _eight = 8;
}

fn blank_lines_do_not_name_phases() {
    let _one = 1;
    let _two = 2;
    let _three = 3;
    let _four = 4;
    let _five = 5;
    let _six = 6;
    let _seven = 7;
    let _eight = 8;
    let _nine = 9;
    let _ten = 10;
    let _eleven = 11;
    let _twelve = 12;
    let _thirteen = 13;
    let _fourteen = 14;
    let _fifteen = 15;
    let _sixteen = 16;

    let _after_boundary = 9;
}

fn mixed_control_flow_is_one_workflow(enabled: bool) {
    let _one = 1;
    let _two = 2;
    let _three = 3;
    let _four = 4;
    if enabled {
        let _inside = 5;
    }
    let _five = 5;
    let _six = 6;
    let _seven = 7;
}

fn nested_block_is_measured_independently() {
    if true {
        let _one = 1;
        let _two = 2;
        let _three = 3;
        let _four = 4;
        let _five = 5;
        let _six = 6;
        let _seven = 7;
        let _eight = 8;
    }
}

fn one_oversized_expression() {
    let _value = Some(1)
        .map(|value| value + 1)
        .map(|value| value + 1)
        .map(|value| value + 1)
        .map(|value| value + 1)
        .map(|value| value + 1)
        .map(|value| value + 1)
        .map(|value| value + 1);
}

fn named_but_oversized() {
    // Prepare all values in one oversized phase.
    let _one = 1;
    let _two = 2;
    let _three = 3;
    let _four = 4;
    let _five = 5;
    let _six = 6;
    let _seven = 7;
    let _eight = 8;
    let _nine = 9;
    let _ten = 10;
    let _eleven = 11;
    let _twelve = 12;
    let _thirteen = 13;
    let _fourteen = 14;
    let _fifteen = 15;
    let _sixteen = 16;
}

fn named_mixed_control_flow(enabled: bool) {
    // Prepare the inputs used by the conditional phase.
    let _one = 1;
    let _two = 2;
    let _three = 3;
    let _four = 4;

    // Apply the conditional work and derive its outputs.
    if enabled {
        let _inside = 5;
    }
    let _five = 5;
    let _six = 6;
    let _seven = 7;
    let _eight = 8;
}

trait DefaultBehavior {
    fn missing_default() {
        let _one = 1;
        let _two = 2;
        let _three = 3;
        let _four = 4;
        let _five = 5;
        let _six = 6;
        let _seven = 7;
        let _eight = 8;
    }
}

struct Service;

impl Service {
    fn missing_method(&self) {
        let _one = 1;
        let _two = 2;
        let _three = 3;
        let _four = 4;
        let _five = 5;
        let _six = 6;
        let _seven = 7;
        let _eight = 8;
    }
}

fn closure_is_independent() {
    let _closure = || {
        let _one = 1;
        let _two = 2;
        let _three = 3;
        let _four = 4;
        let _five = 5;
        let _six = 6;
        let _seven = 7;
        let _eight = 8;
    };
}

macro_rules! generated_function {
    () => {
        fn generated() {
            let _one = 1;
            let _two = 2;
            let _three = 3;
            let _four = 4;
            let _five = 5;
            let _six = 6;
            let _seven = 7;
            let _eight = 8;
        }
    };
}

generated_function!();

fn declarative_mapping(value: u8) -> &'static str {
    match value {
        0 => "zero",
        1 => "one",
        2 => "two",
        3 => "three",
        4 => "four",
        5 => "five",
        6 => "six",
        _ => "many",
    }
}

fn guarded_mapping(value: u8) -> &'static str {
    match value {
        0 if value.is_power_of_two() => "zero",
        1 => "one",
        2 => "two",
        3 => "three",
        4 => "four",
        5 => "five",
        6 => "six",
        _ => "many",
    }
}

struct DeclarativeRecord {
    one: usize,
    two: usize,
    three: usize,
    four: usize,
    five: usize,
    six: usize,
    seven: usize,
    eight: usize,
}

fn declarative_struct_is_one_operation() -> DeclarativeRecord {
    DeclarativeRecord {
        one: 1,
        two: 2,
        three: 3,
        four: 4,
        five: 5,
        six: 6,
        seven: 7,
        eight: 8,
    }
}

fn main() {}
