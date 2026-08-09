#![warn(results_converted_to_options, fallible_values_replaced_with_defaults)]

#[derive(Debug)]
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

fn fail_optional() -> Result<Option<String>, Failure> {
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

fn fallback_conversions() {
    let _mapped = fail().map_or(None, Some);
    let _mapped_lazy = fail().map_or_else(|_| None, Some);
    let _eager = fail_optional().unwrap_or(None);
    let _lazy = fail_optional().unwrap_or_else(|_| None);
    let _defaulted = fail_optional().unwrap_or_default();

    let _ufcs_mapped = Result::map_or(fail(), None, Some);
    let _ufcs_lazy = Result::unwrap_or_else(fail_optional(), |_| None);
    let _explicit_default = fail_optional().unwrap_or(Option::default());
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
    let _reported = fail_optional().unwrap_or_else(|error| {
        eprintln!("load failed: {error:?}");
        None
    });
    let _mapped_reported = fail().map_or_else(
        |error| {
            eprintln!("mapping failed: {error:?}");
            None
        },
        Some,
    );
}

fn main() {
    let _method = method_conversion();
    let _ufcs = ufcs_conversion();
    let _question_mark = question_mark_conversion();
    fallback_conversions();
    accepted();
}
