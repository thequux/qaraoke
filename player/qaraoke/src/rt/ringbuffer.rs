/// lock-free ringbuffer, suitable for use at realtime priority.

use std::sync::atomic::{self, AtomicUsize, Ordering};
use std::ptr;
use std::iter;
use std::slice;
use std::marker;
use std::mem::forget;
use super::AtomicOption;

#[derive(Debug,Eq,Ord,PartialEq,PartialOrd,Copy,Clone)]
pub enum Error {
    /// The other party to the ringbuffer is not available
    Disconnected,
    /// Not enough space or available data to open the requested block
    NoMore,
    /// Larger block requested than will fit in the buffer
    ItWontFit,
}

/// This is the ringbuffer internals; it doesn't enforce any
/// concurrency restrictions. Thus, only Reader and Writer are exposed
/// outside this module.
///
/// Invariants:
///   * capacity is one less than a power of two
///   * 0 ≤ rptr ≤ capacity
///   * 0 ≤ wptr ≤ capacity
///   * One reader, one writer
///   * 1 ≤ capacity
///   * size(self) == (wptr - rptr + capacity + 1) & capacity
///
/// Note that the last point implies that rptr and wptr point to the
/// next element to be read/written, respectively.
pub struct RingBuffer<T> {
    // Capacity must be a power of two
    capacity: usize,
    wptr: AtomicUsize,
    rptr: AtomicUsize,
    buffer: *mut T,
    /// It may be necessary to dispose of a potentially unbounded
    /// number of old ringbuffers at realtime priority.  While this
    /// clearly violates the requirements of hard realtime systems,
    /// this library is designed to be usable from interrupt handlers
    /// as well, where no memory allocation is allowed.
    ///
    /// The dead pointer is used to build up a chain of dead
    /// ringbuffers, such they can be deallocated in the background.
    dead: AtomicOption<RingBuffer<T>>,
    ref_cnt: AtomicUsize,
}

impl <T> RingBuffer<T> {
    fn new<F>(ref_count: usize, capacity: usize, mut default: F) -> *mut Self
        where F: FnMut() -> T
    {
        let mut capacity = capacity.checked_next_power_of_two().expect("Buffer size WAY too fucking large");
        if capacity < 2 {
            // 
            capacity = 2;
        }
        let mut vec = Vec::with_capacity(capacity);
        let buf = vec.as_mut_ptr();
        forget(vec);
        
        // Initialize the buffer
        for i in 0..capacity {
            unsafe {
                ptr::write(buf.offset(i as isize), default());
            }
        }
        Box::into_raw(Box::new(RingBuffer{
            capacity: capacity - 1,
            wptr: AtomicUsize::new(0),
            rptr: AtomicUsize::new(0),
            buffer: buf,
            dead: AtomicOption::new(),
            ref_cnt: AtomicUsize::new(ref_count),
        }))
    }

    fn size(&self) -> usize {
        atomic::fence(Ordering::Acquire);
        (self.capacity - self.rptr.load(Ordering::Relaxed) + self.wptr.load(Ordering::Relaxed) + 1) & self.capacity
    }

    fn available(&self) -> usize {
        self.capacity - self.size()
    }
    
    /// Retrieves two logically contiguous ranges with a total length
    /// of count.  If the requested space is all available in one
    /// segment, the second slice will be nil.
    ///
    /// WARNING: careless use of this function can violate Rust's
    /// invariants WRT aliased &mut references.
    unsafe fn get_range(&self, base: usize, count: usize) -> (&mut [T], &mut [T]) {
        let start = base & self.capacity;
        let end = (start + count)  & self.capacity;
        if start < end {
            return (slice::from_raw_parts_mut(self.buffer.offset(start as isize), end-start),
                    slice::from_raw_parts_mut(self.buffer, 0))
        } else {
            return (slice::from_raw_parts_mut(self.buffer.offset(start as isize), self.capacity + 1 - start),
                    slice::from_raw_parts_mut(self.buffer, end))
        }
    }
}

impl <T> Drop for RingBuffer<T> {
    #[allow(unused_variables)]
    fn drop(&mut self) {
        // Start by deallocating each item from the queue
        println!("Dumping contents");
        let rptr = self.rptr.load(Ordering::Relaxed);
        let wptr = self.wptr.load(Ordering::Relaxed);
        
        if rptr < wptr {
            for i in rptr..wptr {
                unsafe {
                    ptr::drop_in_place(self.buffer.offset(i as isize));
                }
            }
        } else if rptr > wptr {
            for i in rptr..self.capacity+1 {
                unsafe {
                    ptr::drop_in_place(self.buffer.offset(i as isize));
                }
            }
            for i in 0..wptr {
                unsafe {
                    ptr::drop_in_place(self.buffer.offset(i as isize));
                }
            }
        }

        let v = unsafe {Vec::from_raw_parts(self.buffer, 0, self.capacity + 1)}; // and drop it immediately
        self.buffer = ptr::null_mut();
        self.capacity = 0;
    }
}

mod view_type {
    use std::sync::atomic::AtomicUsize;
    use super::RingBuffer;

    pub trait ViewType {
        fn base_ptr<T>(&RingBuffer<T>) -> &AtomicUsize;
        fn limit<T>(&RingBuffer<T>) -> usize;
    }

    pub struct ReaderView{}
    pub struct WriterView{}

    impl ViewType for ReaderView {
        fn base_ptr<T>(buf: &RingBuffer<T>) -> &AtomicUsize {
            &buf.rptr
        }
        fn limit<T>(buf: &RingBuffer<T>) -> usize {
            buf.size()
        }
    }

    impl ViewType for WriterView {
        fn base_ptr<T>(buf: &RingBuffer<T>) -> &AtomicUsize {
            &buf.wptr
        }
        fn limit<T>(buf: &RingBuffer<T>) -> usize {
            buf.available()
        }
    }
}

pub struct View<T, VT: view_type::ViewType> {
    buf: *mut RingBuffer<T>,
    _phantom: marker::PhantomData<VT>,
}

impl <T, VT: view_type::ViewType> View<T, VT> {
    fn check_connected(&self) -> Result<(), Error> {
        if unsafe{&*self.buf}.ref_cnt.load(Ordering::Acquire) > 1 {
            return Ok(());
        } else {
            return Err(Error::Disconnected);
        }
    }

    /// Discards this object on a trash stack. If there are other
    /// references, this simply disconnects itself and lets the other
    /// end clean up.
    pub fn discard(mut self, trash: &AtomicOption<RingBuffer<T>>)  {
        if unsafe{&*self.buf}.ref_cnt.fetch_sub(1, Ordering::AcqRel) <= 1 {
            let boxed = unsafe{Box::from_raw(self.buf)};
            self.buf = ptr::null_mut();
            // We're the only owner
            if let Some(old_trash) = trash.take_box(Ordering::Acquire) {
                if boxed.dead.swap_box(old_trash, Ordering::Relaxed).is_some() {
                    panic!("Tried to discard a RingBuffer that was already in a trash stack");
                }
            }
            if trash.swap_box(boxed, Ordering::Release).is_some() {
                panic!("Race condition on the trash stack");
            }
        }
        // Otherwise, the other end retains ownership
    }
    
    pub fn size(&self) -> usize {
        VT::limit(unsafe{&*self.buf})
    }

    pub fn get_block(&mut self, size: usize) -> Result<MutRange<T>, Error> {
        try!(self.check_connected());
        let buf = unsafe{&*self.buf};
        if size > buf.capacity {
            return Err(Error::ItWontFit);
        } else if size > VT::limit(buf) {
            return Err(Error::NoMore);
        } else {
            let bptr = VT::base_ptr(buf).load(Ordering::Acquire);
            return Ok(MutRange{
                segments: unsafe{buf.get_range(bptr, size)},
                bound_var: VT::base_ptr(buf),
                new_val: (bptr + size) & buf.capacity,
            })
        }
    }
}


// UI for the ringbuffer
pub type Reader<T> = View<T, view_type::ReaderView>;
pub type Writer<T> = View<T, view_type::WriterView>;

pub struct MutRange<'a, T: 'a>{
    segments: (&'a mut [T], &'a mut [T]),
    bound_var: &'a AtomicUsize,
    new_val: usize,
}

impl<'a, T: 'a> MutRange<'a, T> {
    pub fn iter(&mut self) -> MRIter<T> {
        MRIter{iter: self.segments.0.iter_mut().chain(self.segments.1.iter_mut())}
    }
}

impl<'a, T: 'a> Drop for MutRange<'a, T> {
    fn drop(&mut self) {
        self.bound_var.store(self.new_val, Ordering::Release);
    }

}

pub struct MRIter<'a, T: 'a> {
    iter: iter::Chain<slice::IterMut<'a, T>,
                      slice::IterMut<'a, T>>,
}

impl <'a, T: 'a> Iterator for MRIter<'a, T> {
    type Item = &'a mut T;
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

pub fn new<T, F>(size: usize, init: F) -> (Reader<T>, Writer<T>)
    where F: FnMut() -> T
{
    let buffer = RingBuffer::new(2, size, init);
    (Reader{buf: buffer, _phantom: marker::PhantomData},
     Writer{buf: buffer, _phantom: marker::PhantomData})
}

#[cfg(test)]
mod test {
    #[test]
    fn test_ringbuffer_sizes() {
        let rb = unsafe{Box::from_raw(super::RingBuffer::new(1, 6, || 0 as u8))};
        assert_eq!(rb.capacity, 7);
        assert_eq!(rb.available(), 7);
        assert_eq!(rb.size(), 0);
    }

    #[test]
    fn test_ringbuffer_rw() {
        let (mut reader, mut writer) = super::new(16, || 0);
        let mut r = 0;
        let mut w = 0;
        for (n, v) in writer.get_block(6).unwrap().iter().enumerate() {
            *v = n;
            w += 1;
        }
        assert_eq!(w, 6);
        for (n, v) in reader.get_block(4).unwrap().iter().enumerate() {
            assert_eq!(*v, n);
            r += 1;
        }
        assert_eq!(r, 4);
        for (n, v) in writer.get_block(6).unwrap().iter().enumerate() {
            *v = n + 6;
            w += 1;
        }
        assert_eq!(w, 12);
        for (n, v) in reader.get_block(7).unwrap().iter().enumerate() {
            assert_eq!(*v, n + 4);
            r += 1;
        }
        assert_eq!(r, 11);
        for (n, v) in writer.get_block(10).unwrap().iter().enumerate() {
            *v = n + 12;
            w += 1;
        }
        assert_eq!(w, 22);
        for (n, v) in reader.get_block(11).unwrap().iter().enumerate() {
            assert_eq!(*v, n + 11);
            r += 1;
        }
        assert_eq!(r, 22);
        assert_eq!(reader.size(), 0);
        
        
    }
}
