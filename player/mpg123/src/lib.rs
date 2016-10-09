extern crate mpg123_sys;

use std::ptr;

pub use mpg123_sys::Mpg123Error as Error;
pub use mpg123_sys::{Enc, ChannelCount};
use std::marker::PhantomData;

use std::sync::{Once,ONCE_INIT};

static LIBRARY_START: Once = ONCE_INIT;

pub enum SampleBuf {
    Signed8(Vec<i8>),
    Unsigned8(Vec<u8>),
    Signed16(Vec<i16>),
    Unsigned16(Vec<u16>),
    Signed32(Vec<i32>),
    Unsigned32(Vec<u32>),
}

fn init_lib() {
    LIBRARY_START.call_once(|| {
        unsafe { mpg123_sys::mpg123_init() };
    });
}

pub struct Handle<S> {
    handle: *mut mpg123_sys::Mpg123Handle,
    rate: u32,
    channels: u32,
    phantom: PhantomData<S>,
}

fn unless_err<T>(value: T, err: mpg123_sys::c_int) -> Result<T, Error> {
    if err == 0 {
        Ok(value)
    } else {
        Err(err.into())
    }
}

impl <S: SampleFormat> Handle<S> {
    pub fn new() -> Result<Self, Error> {
        init_lib();
        let mut error = 0;
        let handle = unsafe{mpg123_sys::mpg123_new(ptr::null(), &mut error)};
        if error != 0 {
            return Err(error.into());
        }
        let mut handle = Handle{
            handle: handle,
            rate: 0,
            channels: 1,
            phantom: PhantomData,
        };

        // Set up encodings. We force the output format to be S for
        // all formats
        try!(handle.format_none());
        for rate in Self::sample_rates() {
            try!(handle.set_formats(rate, mpg123_sys::CHAN_STEREO | mpg123_sys::CHAN_MONO, S::encoding()));
        }
        
        Ok(handle)
        
    }

    pub fn sample_rates() -> Vec<i32> {
        let mut listptr = ptr::null();
        let mut list_size = 0;
        unsafe {
            mpg123_sys::mpg123_rates(&mut listptr, &mut list_size);
            std::slice::from_raw_parts(listptr, list_size)
        }.iter().map(|x| *x as i32).collect()
    }

    pub fn encodings() -> Vec<mpg123_sys::Enc> {
        let mut listptr = ptr::null();
        let mut list_size = 0;
        unsafe {
            mpg123_sys::mpg123_rates(&mut listptr, &mut list_size);
            std::slice::from_raw_parts(listptr, list_size)
        }.iter().map(|x| mpg123_sys::Enc::from_bits(*x as i32).unwrap()).collect()
    }

    /// Set allowed formats for a given rate
    fn set_formats(&mut self, rate: i32, channels: ChannelCount, encodings: Enc) -> Result<(), Error> {
        unless_err((), unsafe{mpg123_sys::mpg123_format(self.handle, rate, channels.bits(), encodings.bits())})
    }

    fn format_none(&mut self) -> Result<(), Error> {
        unless_err((), unsafe{mpg123_sys::mpg123_format_none(self.handle)})
    }

    pub fn format_supported(&self, rate: i32, encoding: Enc) -> ChannelCount {
        ChannelCount::from_bits(unsafe{mpg123_sys::mpg123_format_support(self.handle, rate, encoding.bits())}).unwrap()
    }

    pub fn open_feed(&mut self) -> Result<(), Error> {
        unless_err((), unsafe{mpg123_sys::mpg123_open_feed(self.handle)})
    }

    pub fn close(&mut self) {
        unsafe {mpg123_sys::mpg123_close(self.handle)};
    }

    pub fn feed(&mut self, bytes: &[u8]) -> Result<(),Error> {
        unless_err((), unsafe {
            mpg123_sys::mpg123_feed(self.handle, bytes.as_ptr(), bytes.len())
        })
    }

    /// Read as many samples as possible from the stream into the
    /// buffer.  Returns the sample rate, the number of channels (1 or
    /// 2), and the number of samples read.
    ///
    /// Note that the term "sample" is ambiguous in audio processing;
    /// here we use it to mean a single value in outbuf, regardless of
    /// the number of channels.
    pub fn shit(&mut self, outbuf: &mut [S]) -> Result<(u32, u32, usize), Error> {
        unsafe {
            let outmem = outbuf.as_ptr() as *mut u8;
            let samplesize = ::std::mem::size_of::<S>();
            let obufsize = outbuf.len() * samplesize;
            let mut done = 0;
            loop {
                match mpg123_sys::mpg123_read(self.handle, outmem, obufsize, &mut done).into() {
                    Error::Ok => break,
                    Error::NewFormat => {
                        self.update_format();
                        continue;
                    },
                    Error::NeedMore => break,
                    Error::Err => return Err(mpg123_sys::mpg123_errcode(self.handle)),
                    err => return Err(err.into()),
                }
            }
            if self.rate == 0 && done != 0 {
                self.update_format();
            }
            let samples = done as usize / samplesize as usize;
            //println!("Got {} samples", samples);
            Ok((self.rate, self.channels, samples))
        }
    }

    fn update_format(&mut self) {
        let mut rate = 0;
        let mut channels = 0;
        let mut encoding = 0;
        unsafe {
            mpg123_sys::mpg123_getformat(self.handle, &mut rate, &mut channels, &mut encoding);
        }
        if self.rate != 0 {
            assert!(Enc::from_bits(encoding).unwrap() == S::encoding());
        }
        println!("Format set to {}/{}/{:?}", rate, channels, encoding);
        self.rate = rate as u32;
        self.channels = if ChannelCount::from_bits(channels).unwrap() == mpg123_sys::CHAN_MONO { 1 } else { 2 };
    }
}

impl <S> Drop for Handle<S> {
    fn drop(&mut self) {
        use std::mem::replace;
        let handle = replace(&mut self.handle, ptr::null_mut());
        unsafe {
            mpg123_sys::mpg123_delete(handle);
        }
    }
}

pub trait SampleFormat {
    fn encoding() -> Enc;
}

impl SampleFormat for i16 {
    fn encoding() -> Enc { mpg123_sys::ENC_SIGNED_16 }
}


#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}
