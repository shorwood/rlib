#![allow(dead_code, rlib::misordered_module_declarations, unknown_lints)]

use std::ops::{Deref, DerefMut, Index, IndexMut};

struct SystemList(Vec<u32>);

impl Deref for SystemList {
    type Target = [u32];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for SystemList {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl AsRef<[u32]> for SystemList {
    fn as_ref(&self) -> &[u32] {
        self.0.as_ref()
    }
}

impl AsMut<[u32]> for SystemList {
    fn as_mut(&mut self) -> &mut [u32] {
        self.0.as_mut()
    }
}

impl Index<usize> for SystemList {
    type Output = u32;

    fn index(&self, index: usize) -> &Self::Output {
        Index::index(&self.0, index)
    }
}

impl IndexMut<usize> for SystemList {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        IndexMut::index_mut(&mut self.0, index)
    }
}

struct DirectIndex(Vec<u32>);

impl Index<usize> for DirectIndex {
    type Output = u32;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl IndexMut<usize> for DirectIndex {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[index]
    }
}

struct Documented(Vec<u32>);

/// This dereference is an authored compatibility contract.
impl Deref for Documented {
    type Target = [u32];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

struct MethodDocumented(Vec<u32>);

impl Deref for MethodDocumented {
    type Target = [u32];

    /// This dereference is an authored compatibility contract.
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

struct Checked(Vec<u32>);

impl Deref for Checked {
    type Target = [u32];

    fn deref(&self) -> &Self::Target {
        assert!(!self.0.is_empty());
        &self.0
    }
}

fn main() {}
