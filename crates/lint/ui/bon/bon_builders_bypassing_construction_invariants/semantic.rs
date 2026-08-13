#![allow(dead_code, misordered_module_declarations, unknown_lints)]

#[derive(bon::Builder)]
pub struct Range {
    start: u32,
    end: u32,
}

impl Range {
    pub fn new(start: u32, end: u32) -> Result<Self, &'static str> {
        if start <= end {
            Ok(Self { start, end })
        } else {
            Err("reversed range")
        }
    }
}

#[derive(bon::Builder)]
pub struct Data {
    value: u32,
}

fn main() {}
