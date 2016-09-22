//#![feature(collections)]
//#![feature(collections_range)]

use std::io::prelude::*;
use std::io;
use std::ops::Index;
use std::ops::{Range,RangeTo,RangeFrom,RangeFull};

// A shiftbuffer works like a VecDeque but guarantees that the data
// contained is slicable. It is also only a unidirectional queue.
pub struct ShiftBuffer {
    content: Vec<u8>,
    /// The maximum block size that needs to be available as a
    /// continuous slice. Also the maxiumum allowed distance between
    /// rptr and wptr
    max_block: usize,
    /// the offset at which to read
    rptr: usize,
    /// the offset at which to write
    wptr: usize,

    /// Offset of buffer[0]
    passed_data: usize,
}    

impl ShiftBuffer {
    pub fn new(max_block: usize) -> Self {
        let mut vec = Vec::with_capacity(max_block * 2);
        unsafe { vec.set_len(max_block * 2); } // The u8's are undefined so far...
        
        ShiftBuffer{
            content: vec,
            max_block: max_block,
            rptr: 0,
            wptr: 0,

            passed_data: 0,
        }
    }

    fn shift(&mut self) {
        use std::ptr;
        // This condition guarantees at most a single copy per element.
        if self.rptr >= self.max_block {
            let buf = self.content.as_mut_ptr();
            unsafe {ptr::copy_nonoverlapping(buf.offset(self.rptr as isize), buf, self.len());}
            self.wptr = self.len();
            self.rptr = 0;
        }
    }

    pub fn len(&self) -> usize {
        self.wptr - self.rptr
    }

    pub fn is_empty(&self) -> bool {
        self.wptr == self.rptr
    }

    pub fn fill<R: Read>(&mut self, reader: &mut R, len: usize) -> io::Result<usize> {
        // We only attempt to shift when writing, to reduce needless
        // shifts Further, shift only actually shifts when it can
        // shift by more than max_block
        self.shift();
        if len + self.len() > self.max_block {
            panic!("Tried to read too much into a buffer");
        }
        reader.read(&mut self.content[self.wptr..self.wptr+len]).map(|count| {self.wptr += count; self.passed_data += count; count})
    }

    pub fn fill_to<R: Read>(&mut self, reader: &mut R, target: usize) -> io::Result<usize> {
        assert!(target <= self.max_block);
        let mut read = 0;
        while self.len() < target {
            let len = self.len();
            let count = try!(self.fill(reader, target - len));
            read += count;
            if count == 0 {
                return Ok(read);
            }
        }
        Ok(read)
    }

    pub fn fill_max<R: Read>(&mut self, reader: &mut R) -> io::Result<usize> {
        let max_block = self.max_block;
        self.fill_to(reader, max_block)
    }

    pub fn consume(&mut self, amount: usize) -> &[u8] {
        if amount > self.len() {
            panic!("Consumed more buffer space than was filled");
        }
        let res = &self.content[self.rptr..self.rptr + amount];
        self.rptr += amount;
        res
    }

    pub fn offset(&self) -> usize {
        self.passed_data
    }
}


impl Index<usize> for ShiftBuffer {
    type Output = u8;

    fn index(&self, idx: usize) -> &u8 {
        if idx + self.rptr >= self.wptr {
            panic!("Index out of bounds");
        }
        &self.content[idx + self.rptr]
    }
}

impl Index<Range<usize>> for ShiftBuffer {
    type Output = [u8];
    
    fn index(&self, idx: Range<usize>) -> &[u8] {
        let min = idx.start + self.rptr;
        let max = idx.end + self.rptr;
        
        if min >= self.wptr || max > self.wptr || min > max {
            panic!("Index out of bounds");
        }
        &self.content[min..max]
    }
}

impl Index<RangeTo<usize>> for ShiftBuffer {
    type Output = [u8];
    
    fn index(&self, idx: RangeTo<usize>) -> &[u8] {
        self.index(0..idx.end)
    }
}

impl Index<RangeFrom<usize>> for ShiftBuffer {
    type Output = [u8];
    
    fn index(&self, idx: RangeFrom<usize>) -> &[u8] {
        self.index(idx.start..self.len())
    }
}
impl Index<RangeFull> for ShiftBuffer {
    type Output = [u8];
    
    fn index(&self, _: RangeFull) -> &[u8] {
        &self.content[self.rptr..self.wptr]
    }
}
