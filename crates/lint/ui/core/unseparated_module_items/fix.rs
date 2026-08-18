// run-rustfix
// rustfix-only-machine-applicable

#![allow(dead_code)]
#![warn(rlib::unseparated_module_items)]

struct Repository;
#[derive(Default)]
struct MemoryRepository;
impl MemoryRepository {
    fn open() -> Self {
        Self
    }
}
fn close() {}

mod compact_first {}
mod compact_second {}

fn main() {}
