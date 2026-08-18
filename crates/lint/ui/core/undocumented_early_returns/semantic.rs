#![warn(rlib::undocumented_early_returns)]
#![allow(dead_code, rlib::method_like_free_functions, unreachable_code)]

struct Progress {
    value: usize,
}

fn undocumented_if(enabled: bool, progress: &mut Progress) {
    if !enabled {
        return;
    }
    progress.value += 1;
}

fn undocumented_let_else(value: Option<usize>, progress: &mut Progress) {
    let Some(_) = value else {
        return;
    };
    progress.value += 1;
}

fn undocumented_match(value: Option<usize>, progress: &mut Progress) {
    match value {
        Some(_) => return,
        None => progress.value += 1,
    }
    consume();
}

fn return_comment_is_too_late(enabled: bool, progress: &mut Progress) {
    if !enabled {
        // Disabled operations preserve their progress.
        return;
    }
    progress.value += 1;
}

fn shared_condition_has_one_finding(enabled: bool) {
    if enabled {
        return;
    } else {
        return;
    }
    consume();
}

fn closure_condition_is_checked() {
    let callback = |enabled: bool| {
        if !enabled {
            return;
        }
        consume();
    };
    callback(false);
}

fn documented_if(enabled: bool, progress: &mut Progress) {
    // Disabled operations preserve their progress.
    if !enabled {
        return;
    }
    progress.value += 1;
}

fn documented_let_else(value: Option<usize>, progress: &mut Progress) {
    // Missing values cannot contribute progress.
    let Some(_) = value else {
        return;
    };
    progress.value += 1;
}

fn documented_match(value: Option<usize>, progress: &mut Progress) {
    match value {
        // Present values end this operation immediately.
        Some(_) => return,
        None => progress.value += 1,
    }
    consume();
}

fn direct_return_is_excluded() {
    return;
    consume();
}

fn closure_direct_return_is_excluded() {
    let callback = || {
        return;
        consume();
    };
    callback();
}

fn terminal_if_is_excluded(enabled: bool) {
    if enabled {
        return;
    }
}

fn terminal_let_else_is_excluded(value: Option<usize>) {
    let Some(_) = value else {
        return;
    };
}

fn terminal_match_is_excluded(value: Option<usize>) {
    match value {
        Some(_) => return,
        None => return,
    }
}

macro_rules! generated_function {
    () => {
        fn generated(enabled: bool) {
            if enabled {
                return;
            }
            consume();
        }
    };
}

generated_function!();

fn consume() {}

fn main() {}
