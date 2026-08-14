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

impl<'a> IntoIterator for &'a mut SystemList {
    type Item = &'a mut u32;
    type IntoIter = std::slice::IterMut<'a, u32>;

    fn into_iter(self) -> Self::IntoIter {
        (&mut self.0).into_iter()
    }
}

struct NamedGeneric<T> {
    values: Vec<T>,
}

impl<T> IntoIterator for NamedGeneric<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.values.into_iter()
    }
}

struct Documented(Vec<u32>);

/// This iteration implementation is an authored compatibility contract.
impl IntoIterator for Documented {
    type Item = u32;
    type IntoIter = std::vec::IntoIter<u32>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

struct MethodDocumented(Vec<u32>);

impl IntoIterator for MethodDocumented {
    type Item = u32;
    type IntoIter = std::vec::IntoIter<u32>;

    /// This iteration method is an authored compatibility contract.
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
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
