use std::ops::{Index, IndexMut};

/// Unsafe array that can be sent between threads.
/// It allows for independent read/write in threads but is vulnerable to race conditions, use-after-free and etc.
#[derive(Clone, Copy)]
pub struct UnsafeMultithreadedArray<T> {
    ptr: *mut T
}

impl<T> UnsafeMultithreadedArray<T> {
    pub fn new(data: &mut [T]) -> Self {
        Self {
            ptr: data.as_mut_ptr()
        }
    }
}

impl<T> Index<usize> for UnsafeMultithreadedArray<T> {
    type Output = T;
    fn index(&self, index: usize) -> &Self::Output {
        unsafe {
            &*self.ptr.add(index)
        }
    }
}

impl<T> IndexMut<usize> for UnsafeMultithreadedArray<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        unsafe {
            &mut *self.ptr.add(index)
        }
    }
}

unsafe impl<T> Send for UnsafeMultithreadedArray<T> {}
unsafe impl<T> Sync for UnsafeMultithreadedArray<T> {}