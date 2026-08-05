#![warn(missing_code_phase_comments)]
#![allow(dead_code, misordered_module_declarations)]
// edition:2024

fn missing() {
    let _one = 1;
    let _two = 2;
    let _three = 3;
    let _four = 4;
    let _five = 5;
    let _six = 6;
    let _seven = 7;
    let _eight = 8;
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
}

#[allow(missing_code_phase_comments)]
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

fn main() {}
