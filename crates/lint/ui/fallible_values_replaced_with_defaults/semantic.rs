#![warn(fallible_values_replaced_with_defaults)]

struct Failure(&'static str);

type LoadResult = Result<String, Failure>;

fn fail() -> LoadResult {
    Err(Failure("unavailable"))
}

fn fail_optional() -> Result<Option<String>, Failure> {
    Err(Failure("unavailable"))
}

macro_rules! generated_default {
    () => {
        fail().unwrap_or_default()
    };
}

fn default_operations() {
    let _dedicated = fail().unwrap_or_default();
    let _eager = fail().unwrap_or(String::default());
    let _lazy = fail().unwrap_or_else(|_| String::default());

    let _ufcs_dedicated = Result::unwrap_or_default(fail());
    let _ufcs_eager = Result::unwrap_or(fail(), String::default());
    let _ufcs_lazy = Result::unwrap_or_else(fail(), |_| String::default());
}

fn accepted() {
    let _option = Option::<String>::None.unwrap_or_default();
    let _error_aware = fail().unwrap_or_else(|error| error.0.to_owned());
    let _reported = fail().unwrap_or_else(|error| {
        eprintln!("load failed: {}", error.0);
        String::default()
    });
    let _option_result = fail_optional().unwrap_or_default();
    let _explicit = match fail() {
        Ok(value) => value,
        Err(_) => String::default(),
    };
    let _generated = generated_default!();
}

fn main() {
    default_operations();
    accepted();
}
