#![warn(undocumented_early_returns)]
#![allow(dead_code, method_like_free_functions, unreachable_code)]
// edition:2024

struct Progress {
    value: usize,
}

fn undocumented_guard(enabled: bool, progress: &mut Progress) {
    progress.value += 1;
    if !enabled {
        return;
    }
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
}

fn undocumented_let_else(value: Option<usize>, progress: &mut Progress) {
    progress.value += 1;
    let Some(_) = value else {
        return;
    };
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
}

fn undocumented_match(value: Option<usize>, progress: &mut Progress) {
    progress.value += 1;
    match value {
        Some(_) => return,
        None => consume(),
    }
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
}

fn undocumented_direct(progress: &mut Progress) {
    progress.value += 1;
    return;
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
}

fn documented_guard(enabled: bool, progress: &mut Progress) {
    progress.value += 1;

    // Stop when the caller disabled this operation.
    if !enabled {
        return;
    }
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
}

fn documented_return(enabled: bool, progress: &mut Progress) {
    progress.value += 1;
    if !enabled {
        // Preserve the disabled state without performing work.
        return;
    }
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
}

fn malformed_comment_is_owned_elsewhere(enabled: bool, progress: &mut Progress) {
    progress.value += 1;

    // this malformed explanation is intentionally detached

    if !enabled {
        return;
    }
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
}

fn short_continuation_is_local(enabled: bool, progress: &mut Progress) {
    progress.value += 1;
    if !enabled {
        return;
    }
    consume();
}

fn entry_guard_precedes_mutation(enabled: bool, progress: &mut Progress) {
    if !enabled {
        return;
    }
    progress.value += 1;
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
    consume();
}

fn terminal_return() {
    return;
}

fn terminal_match(value: Option<usize>) {
    match value {
        Some(_) => return,
        None => return,
    }
}

fn closure_return_is_independent() {
    let _callback = || {
        return;
    };
    consume();
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

#[allow(undocumented_early_returns)]
fn explicitly_allowed(enabled: bool) {
    if enabled {
        return;
    }
    consume();
}

fn consume() {}

fn main() {}
