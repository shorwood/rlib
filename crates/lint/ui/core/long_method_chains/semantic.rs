#![warn(long_method_chains)]
#![allow(dead_code, misordered_module_declarations)]

fn too_long(values: &[String]) -> Vec<String> {
    values
        .iter()
        .filter(|value| !value.is_empty())
        .cloned()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .collect()
}

fn exact_limit(values: &[String]) -> Vec<String> {
    values
        .iter()
        .filter(|value| !value.is_empty())
        .cloned()
        .map(|value| value.trim().to_owned())
        .collect()
}

fn intermediary(values: &[String]) -> Vec<String> {
    let populated = values.iter().filter(|value| !value.is_empty());
    populated.cloned().collect()
}

fn question_mark_breaks(value: Option<String>) -> Option<usize> {
    let value = value.as_ref().map(String::as_str)?;
    Some(value.trim().chars().count())
}

fn closure_is_independent(values: &[String]) {
    let _closure = || {
        values
            .iter()
            .filter(|value| !value.is_empty())
            .cloned()
            .collect::<Vec<_>>()
    };
}

macro_rules! generated_chain {
    () => {
        fn generated_chain(values: &[String]) -> Vec<String> {
            values
                .iter()
                .filter(|value| !value.is_empty())
                .cloned()
                .collect()
        }
    };
}

generated_chain!();

#[allow(long_method_chains)]
fn explicitly_allowed(values: &[String]) -> Vec<String> {
    values
        .iter()
        .filter(|value| !value.is_empty())
        .cloned()
        .collect()
}

fn consume(_: Vec<String>) {}

fn nested_as_argument(values: &[String]) {
    consume(
        values
            .iter()
            .filter(|value| !value.is_empty())
            .cloned()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .collect(),
    );
}

fn main() {}
