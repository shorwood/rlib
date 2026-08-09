pub struct ThreadPoolBuilder;

impl ThreadPoolBuilder {
    pub fn new() -> Self {
        Self
    }

    pub fn num_threads(self, _: usize) -> Self {
        self
    }
}
