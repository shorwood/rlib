#![warn(discarded_results)]

struct Failure(&'static str);

fn fail() -> Result<String, Failure> {
    Err(Failure("unavailable"))
}

fn custom_drop<T>(_value: T) {}

macro_rules! generated_discard {
    () => {
        let _ = fail();
    };
}

fn discarded() {
    let _ = fail();
    _ = fail();
    drop(fail());
    std::mem::drop(fail());
}

fn explicit_policy() -> Result<(), Failure> {
    match fail() {
        Ok(value) => println!("{value}"),
        Err(error) => println!("{}", error.0),
    }
    custom_drop(fail());
    generated_discard!();
    fail()?;
    Ok(())
}

fn main() {
    discarded();
    let _handled = explicit_policy();
}
