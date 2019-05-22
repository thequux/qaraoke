// Copyright (c) 2015 The Rust Project Developers

// Permission is hereby granted, free of charge, to any
// person obtaining a copy of this software and associated
// documentation files (the "Software"), to deal in the
// Software without restriction, including without
// limitation the rights to use, copy, modify, merge,
// publish, distribute, sublicense, and/or sell copies of
// the Software, and to permit persons to whom the Software
// is furnished to do so, subject to the following
// conditions:

// The above copyright notice and this permission notice
// shall be included in all copies or substantial portions
// of the Software.

// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
// ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
// TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
// PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
// SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
// CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
// IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

use std::sync::atomic::{AtomicPtr, Ordering};
use std::ptr;

unsafe impl<T: Send> Send for AtomicOption<T> {}
unsafe impl<T: Send> Sync for AtomicOption<T> {}

#[derive(Debug)]
pub struct AtomicOption<T> {
    inner: AtomicPtr<T>,
}

impl<T> AtomicOption<T> {
    pub fn new() -> AtomicOption<T> {
        AtomicOption { inner: AtomicPtr::new(ptr::null_mut()) }
    }

    fn swap_inner(&self, ptr: *mut T, order: Ordering) -> Option<Box<T>> {
        let old = self.inner.swap(ptr, order);
        if old.is_null() {
            None
        } else {
            Some(unsafe { Box::from_raw(old) })
        }
    }

    // allows re-use of allocation
    pub fn swap_box(&self, t: Box<T>, order: Ordering) -> Option<Box<T>> {
        self.swap_inner(Box::into_raw(t), order)
    }

    pub fn swap(&self, t: T, order: Ordering) -> Option<T> {
        self.swap_box(Box::new(t), order).map(|old| *old)
    }

    pub fn take_box(&self, order: Ordering) -> Option<Box<T>> {
        self.swap_inner(ptr::null_mut(), order)
    }

    pub fn take(&self, order: Ordering) -> Option<T> {
        self.take_box(order).map(|old| *old)
    }
}

impl<T> Drop for AtomicOption<T> {
    fn drop(&mut self) {
        self.take_box(Ordering::Relaxed);
    }
}
