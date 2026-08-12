pub trait StreamExt: Sized {
    fn buffer_unordered(self, _: usize) -> Self {
        self
    }
}

impl<T> StreamExt for T {}
