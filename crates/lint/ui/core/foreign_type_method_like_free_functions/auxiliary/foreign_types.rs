#![feature(decl_macro)]

pub struct Ambient;
pub struct Document;
pub struct ExternalItem;
pub struct ExternalType;
pub struct Registry;

pub macro external_helper() {
    pub fn generated_externally(item: &ExternalItem) {}
}
