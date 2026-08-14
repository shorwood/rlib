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
            let _nine = value + 9;
            let _ten = value + 10;
            let _eleven = value + 11;
            let _twelve = value + 12;
            let _thirteen = value + 13;
            let _fourteen = value + 14;
            let _fifteen = value + 15;
            let _sixteen = value + 16;
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
            let _eight = value + 8;
            let _nine = value + 9;
            let _ten = value + 10;
            let _eleven = value + 11;
            let _twelve = value + 12;
            let _thirteen = value + 13;
            let _fourteen = value + 14;
            let _fifteen = value + 15;
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
            let _nine = value + 9;
            let _ten = value + 10;
            let _eleven = value + 11;
            let _twelve = value + 12;
            let _thirteen = value + 13;
            let _fourteen = value + 14;
            let _fifteen = value + 15;
            let _sixteen = value + 16;
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
                    let _nine = value + 9;
                    let _ten = value + 10;
                    let _eleven = value + 11;
                    let _twelve = value + 12;
                    let _thirteen = value + 13;
                    let _fourteen = value + 14;
                    let _fifteen = value + 15;
                    let _sixteen = value + 16;
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
            let _nine = value + 9;
            let _ten = value + 10;
            let _eleven = value + 11;
            let _twelve = value + 12;
            let _thirteen = value + 13;
            let _fourteen = value + 14;
            let _fifteen = value + 15;
            let _sixteen = value + 16;
        }
        None => {}
    }
}

fn many_statements_on_one_line(value: Option<u8>) {
    match value {
        Some(value) => { let _01 = value + 1; let _02 = value + 2; let _03 = value + 3; let _04 = value + 4; let _05 = value + 5; let _06 = value + 6; let _07 = value + 7; let _08 = value + 8; let _09 = value + 9; let _10 = value + 10; let _11 = value + 11; let _12 = value + 12; let _13 = value + 13; let _14 = value + 14; let _15 = value + 15; let _16 = value + 16; }
        None => {}
    }
}

fn main() {}
