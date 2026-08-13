#![allow(dead_code, misordered_module_declarations, unknown_lints)]

struct SystemList(Vec<u32>);

impl IntoIterator for SystemList {
    type Item = u32;
    type IntoIter = std::vec::IntoIter<u32>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIterator::into_iter(self.0)
    }
}

impl<'a> IntoIterator for &'a SystemList {
    type Item = &'a u32;
    type IntoIter = std::slice::Iter<'a, u32>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIterator::into_iter(&self.0)
    }
}

struct Filtered(Vec<u32>);

impl IntoIterator for Filtered {
    type Item = u32;
    type IntoIter = std::vec::IntoIter<u32>;

    fn into_iter(mut self) -> Self::IntoIter {
        self.0.retain(|value| *value != 0);
        self.0.into_iter()
    }
}

fn main() {}
