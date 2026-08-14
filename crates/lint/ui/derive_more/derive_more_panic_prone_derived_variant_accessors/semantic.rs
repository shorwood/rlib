#![allow(dead_code, unknown_lints)]

#[derive(derive_more::Unwrap)]
#[unwrap(owned, ref, ref_mut)]
enum Response {
    Ok(String),
    Error(String),
    Pending,
}

fn unchecked(response: Response) -> String {
    response.unwrap_ok()
}

fn established() -> String {
    Response::Ok(String::new()).unwrap_ok()
}

fn established_through_block() -> String {
    ({ Response::Ok(String::new()) }).unwrap_ok()
}

fn established_unit() {
    Response::Pending.unwrap_pending();
}

fn wrong_direct_variant() -> String {
    Response::Error(String::new()).unwrap_ok()
}

fn unchecked_ref(response: &Response) -> &String {
    response.unwrap_ok_ref()
}

fn unchecked_mut(response: &mut Response) -> &mut String {
    response.unwrap_ok_mut()
}

macro_rules! generated_unwrap {
    ($response:expr) => {
        $response.unwrap_ok()
    };
}

fn generated_call(response: Response) -> String {
    generated_unwrap!(response)
}

struct Custom;

impl Custom {
    fn unwrap_ok(self) {}

    fn exercise(self) {
        self.unwrap_ok();
    }
}

fn main() {}
