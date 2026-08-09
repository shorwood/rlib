#![warn(results_converted_to_options)]

struct Failure;

type LoadResult = Result<String, Failure>;

struct CustomResult;

impl CustomResult {
    fn ok(self) -> Option<String> {
        None
    }
}

fn fail() -> LoadResult {
    Err(Failure)
}

macro_rules! generated_conversion {
    () => {
        fail().ok()
    };
}

fn method_conversion() -> Option<String> {
    fail().ok()
}

fn ufcs_conversion() -> Option<String> {
    Result::ok(fail())
}

fn question_mark_conversion() -> Option<usize> {
    let value = fail().ok()?;
    Some(value.len())
}

fn explicit_policy() -> Option<String> {
    match fail() {
        Ok(value) => Some(value),
        Err(_) => None,
    }
}

fn accepted() {
    let _custom = CustomResult.ok();
    let _generated = generated_conversion!();
    let _explicit = explicit_policy();
}

fn main() {
    let _method = method_conversion();
    let _ufcs = ufcs_conversion();
    let _question_mark = question_mark_conversion();
    accepted();
}
