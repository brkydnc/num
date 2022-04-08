use std::sync::atomic::{AtomicUsize, Ordering};

pub type Id = usize;

pub struct IdGenerator(AtomicUsize);

impl IdGenerator {
    pub fn new() -> Self {
        Self(AtomicUsize::new(0))
    }

    pub fn next(&self) -> Id {
        self.0.fetch_add(1, Ordering::Relaxed)
    }
}
