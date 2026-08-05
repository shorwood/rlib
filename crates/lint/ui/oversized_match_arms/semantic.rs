#![warn(oversized_match_arms)]
#![allow(dead_code, misordered_module_declarations)]

fn oversized(value: Option<u8>) {
    match value {
        Some(value) => {
            let _one = value + 1;
            let _two = value + 2;
            let _three = value + 3;
            let _four = value + 4;
            let _five = value + 5;
            let _six = value + 6;
            let _seven = value + 7;
            let _eight = value + 8;
        }
        None => {}
    }
}

fn exact_limit(value: Option<u8>) {
    match value {
        Some(value) => {
            let _one = value + 1;
            let _two = value + 2;
            let _three = value + 3;
            let _four = value + 4;
            // Comments and blank lines do not count.

            let _five = value + 5;
            let _six = value + 6;
            let _seven = value + 7;
        }
        None => {}
    }
}

fn closure_is_independent(value: Option<u8>) {
    let _closure = || match value {
        Some(value) => {
            let _one = value + 1;
            let _two = value + 2;
            let _three = value + 3;
            let _four = value + 4;
            let _five = value + 5;
            let _six = value + 6;
            let _seven = value + 7;
            let _eight = value + 8;
        }
        None => {}
    };
}

macro_rules! generated_match {
    () => {
        fn generated_match(value: Option<u8>) {
            match value {
                Some(value) => {
                    let _one = value + 1;
                    let _two = value + 2;
                    let _three = value + 3;
                    let _four = value + 4;
                    let _five = value + 5;
                    let _six = value + 6;
                    let _seven = value + 7;
                    let _eight = value + 8;
                }
                None => {}
            }
        }
    };
}

generated_match!();

#[allow(oversized_match_arms)]
fn explicitly_allowed(value: Option<u8>) {
    match value {
        Some(value) => {
            let _one = value + 1;
            let _two = value + 2;
            let _three = value + 3;
            let _four = value + 4;
            let _five = value + 5;
            let _six = value + 6;
            let _seven = value + 7;
            let _eight = value + 8;
        }
        None => {}
    }
}

fn main() {}
