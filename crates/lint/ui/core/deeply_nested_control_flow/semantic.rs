#![warn(deeply_nested_control_flow)]
#![allow(dead_code, misordered_module_declarations)]

fn too_deep(first: bool, second: bool, third: bool) {
    if first {
        match second {
            true => loop {
                if third {
                    break;
                }
                break;
            },
            false => {}
        }
    }
}

fn exact_depth(first: bool, second: bool) {
    if first {
        while second {
            break;
        }
    }
}

fn guardable_depth_still_counts(first: bool, second: bool, third: bool) {
    if first {
        while second {
            if third {}
        }
    }
}

fn else_if_is_one_level(first: bool, second: bool, third: bool) {
    if first {
    } else if second {
    } else if third {
    }
}

fn closure_is_independent(first: bool, second: bool, third: bool) {
    let _closure = || {
        if first {
            while second {
                if third {}
            }
        }
    };
}

macro_rules! generated_nesting {
    () => {
        fn generated_nesting(first: bool, second: bool, third: bool) {
            if first {
                while second {
                    if third {}
                }
            }
        }
    };
}

generated_nesting!();

#[allow(deeply_nested_control_flow)]
fn explicitly_allowed(first: bool, second: bool, third: bool) {
    if first {
        while second {
            match third {
                true => {}
                false => {}
            }
        }
    }
}

fn main() {}
