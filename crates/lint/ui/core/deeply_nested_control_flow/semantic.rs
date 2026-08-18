#![warn(rlib::deeply_nested_control_flow)]
#![allow(dead_code, rlib::misordered_module_declarations)]

fn too_deep(first: bool, second: bool, third: bool, fourth: bool) {
    if first {
        match second {
            true => loop {
                if third {
                    if fourth {
                        break;
                    }
                }
                break;
            },
            false => {}
        }
    }
}

fn exact_depth(first: bool, second: bool, third: bool) {
    if first {
        while second {
            match third {
                true => loop {
                    break;
                },
                false => {}
            }
        }
    }
}

fn guardable_depth_still_counts(first: bool, second: bool, third: bool, fourth: bool) {
    if first {
        while second {
            match third {
                true => loop {
                    if fourth {}
                },
                false => {}
            }
        }
    }
}

#[warn(rlib::needlessly_nested_control_flow)]
fn guardable_depth_is_deferred(first: bool, second: bool, third: bool, fourth: bool) {
    if first {
        while second {
            match third {
                true => loop {
                    if fourth {}
                },
                false => {}
            }
        }
    }
}

fn else_if_is_one_level(first: bool, second: bool, third: bool) {
    if first {
    } else if second {
    } else if third {
    }
}

fn closure_is_independent(first: bool, second: bool, third: bool, fourth: bool) {
    let _closure = || {
        if first {
            while second {
                if third {
                    match fourth {
                        true => loop {
                            break;
                        },
                        false => {}
                    }
                }
            }
        }
    };
}

macro_rules! generated_nesting {
    () => {
        fn generated_nesting(first: bool, second: bool, third: bool, fourth: bool) {
            if first {
                while second {
                    if third {
                        match fourth {
                            true => loop {
                                break;
                            },
                            false => {}
                        }
                    }
                }
            }
        }
    };
}

generated_nesting!();

#[allow(rlib::deeply_nested_control_flow)]
fn explicitly_allowed(first: bool, second: bool, third: bool, fourth: bool) {
    if first {
        while second {
            match third {
                true => loop {
                    if fourth {
                        break;
                    }
                },
                false => {}
            }
        }
    }
}

fn main() {}
