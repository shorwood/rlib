#![warn(revalidated_string_parameters)]
#![allow(dead_code, misordered_module_declarations)]

fn validate_email(_email: &str) -> Result<(), ()> {
    Ok(())
}

fn send_invitation(email: &str) -> Result<(), ()> {
    validate_email(email)?;
    Ok(())
}

fn subscribe(email: &str) -> Result<(), ()> {
    validate_email(email)?;
    Ok(())
}

fn publish(slug: &str) -> Result<(), ()> {
    if slug.is_empty() {
        return Err(());
    }
    Ok(())
}

fn archive(slug: &str) -> Result<(), ()> {
    if slug.is_empty() {
        return Err(());
    }
    Ok(())
}

fn cached_route(route: &str) -> Result<(), ()> {
    if route == "cached" {
        return Ok(());
    }
    Ok(())
}

fn local_route(route: &str) -> Result<(), ()> {
    if route == "local" {
        return Ok(());
    }
    Ok(())
}

fn validate_token(_token: &str) -> Result<(), ()> {
    Ok(())
}

fn one_consumer(token: &str) -> Result<(), ()> {
    validate_token(token)?;
    Ok(())
}

fn validate_key(_key: &str) -> Result<(), ()> {
    Ok(())
}

fn normalize_key(key: &str) -> String {
    key.to_owned()
}

fn first_key_consumer(key: &str) -> Result<(), ()> {
    validate_key(key)?;
    Ok(())
}

fn second_key_consumer(key: &str) -> Result<(), ()> {
    validate_key(key)?;
    Ok(())
}

mod boundary {
    fn parse_email(source: &str) -> Result<String, ()> {
        super::validate_email(source)?;
        Ok(source.to_owned())
    }
}

fn main() {}
