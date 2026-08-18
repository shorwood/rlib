// aux-build: string_domain_external_macro.rs

#![warn(rlib::stringly_typed_domain_function_families)]
#![allow(dead_code, rlib::misordered_module_declarations)]

extern crate string_domain_external_macro;

use std::path::PathBuf;

use string_domain_external_macro::external_domain_family;

fn validate_slug(_slug: &str) -> Result<(), ()> {
    Ok(())
}

fn normalize_slug(slug: &str) -> String {
    slug.to_lowercase()
}

fn slug_path(slug: &str) -> PathBuf {
    PathBuf::from(slug)
}

struct User {
    email: String,
}

fn validate_email(_email: &str) -> Result<(), ()> {
    Ok(())
}

fn format_message(message: &str) -> String {
    message.to_owned()
}

fn parse_name(source: &str) -> Result<String, ()> {
    Ok(source.to_owned())
}

struct AccountKey(String);

fn validate_account_key(_account_key: &str) -> Result<(), ()> {
    Ok(())
}

fn normalize_account_key(account_key: &str) -> String {
    account_key.to_owned()
}

mod first {
    pub fn normalize_key(key: &str) -> String {
        key.to_owned()
    }
}

mod second {
    pub fn validate_key(_key: &str) -> Result<(), ()> {
        Ok(())
    }
}

struct SlugText;

impl SlugText {
    fn normalize_slug(slug: &str) -> String {
        slug.to_owned()
    }
}

external_domain_family!();

fn main() {}
