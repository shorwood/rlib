pub mod sync {
    pub struct Semaphore;

    impl Semaphore {
        pub fn new(_: usize) -> Self {
            Self
        }
    }

    pub mod mpsc {
        pub fn channel<T>(_: usize) -> (Sender<T>, Receiver<T>) {
            (Sender(core::marker::PhantomData), Receiver(core::marker::PhantomData))
        }

        pub struct Sender<T>(core::marker::PhantomData<T>);
        pub struct Receiver<T>(core::marker::PhantomData<T>);
    }
}
