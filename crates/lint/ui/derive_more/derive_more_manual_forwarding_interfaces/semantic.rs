#![allow(dead_code, misordered_module_declarations, unknown_lints)]

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

struct Checked(Vec<u32>);

impl Deref for Checked {
    type Target = [u32];

    fn deref(&self) -> &Self::Target {
        assert!(!self.0.is_empty());
        &self.0
    }
}

fn main() {}
