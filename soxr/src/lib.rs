pub extern crate soxr_sys as sys;
use std::error;
use std::fmt;
use std::ptr;
use std::ffi;
use std::os;
use std::marker::PhantomData;

#[derive(Debug)]
pub struct Error {
    err: String,
}

unsafe fn from_soxr_error(err: sys::soxr_error_t) -> Result<(), Error> {
    if !err.is_null() {
        Err(ffi::CStr::from_ptr(err).into())
    } else {
        Ok(())
    }
}


impl <'a> From<&'a ffi::CStr> for Error {
    fn from(err: &'a ffi::CStr) -> Error {
        Error{err: err.to_string_lossy().into_owned()}
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl error::Error for Error {
    fn description(&self) -> &str { &self.err }
    fn cause(&self) -> Option<&error::Error> { None }
}

pub trait SoxrFormat {
    fn soxr_datatype() -> sys::soxr_datatype_t;
    fn n_channels() -> u32;
}

impl SoxrFormat for f32 {
    fn soxr_datatype() -> sys::soxr_datatype_t { sys::soxr_datatype_t::SOXR_FLOAT32_I }
    fn n_channels() -> u32 { 1 }
}

impl <T: SoxrFormat> SoxrFormat for [T;2] {
    fn soxr_datatype() -> sys::soxr_datatype_t { T::soxr_datatype() }
    fn n_channels() -> u32 { T::n_channels() * 2 }
}

pub struct Soxr<I,O> {
    handle: sys::soxr_t,
    _phantom: PhantomData<(I,O)>,
}

// I and O must match channel count
pub struct SoxrBuilder<I, O> {
    rate: f64, // input / output
    io_spec: sys::soxr_io_spec_t,
    quality_spec: sys::soxr_quality_spec_t,
    runtime_spec: sys::soxr_runtime_spec_t,
    _phantom: PhantomData<(I,O)>,
}

impl<I: SoxrFormat, O: SoxrFormat> SoxrBuilder<I,O> {
    pub fn new() -> Self {
        SoxrBuilder{
            rate: 0.,
            io_spec: sys::soxr_io_spec_t{
                itype: I::soxr_datatype(),
                otype: O::soxr_datatype(),
                scale: 1.,
                e: ptr::null_mut(),
                flags: sys::soxr_io_flags::empty(),
            },
            quality_spec: unsafe{sys::soxr_quality_spec(sys::SOXR_HQ, 0)},
            runtime_spec: unsafe{sys::soxr_runtime_spec(1)},
            _phantom: PhantomData,
        }
    }

    pub fn with_scale(mut self, scale: f64) -> Self {
        self.io_spec.scale = scale;
        self
    }

    pub fn set_quality(mut self, recipe: os::raw::c_ulong, flags: sys::soxr_quality_flags) -> Self {
        self.quality_spec = unsafe{sys::soxr_quality_spec(recipe,
                                                          flags.bits() as os::raw::c_ulong)};
        self
    }

    pub fn set_threads(mut self, count: u32) -> Self {
        self.runtime_spec = unsafe{sys::soxr_runtime_spec(count as std::os::raw::c_uint)};
        self
    }
    
    pub fn build(self) -> Result<Soxr<I,O>, Error> {
        let mut err = ptr::null();
        let rate = if self.rate == 0.0 {
            (0.,0.)
        } else {
            (self.rate, 1.)
        };

        if I::n_channels() != O::n_channels() {
            return Err(Error{err: "Input and output types have a different number of channels".to_owned()})
        }
        let res = unsafe {
            sys::soxr_create(
                rate.0, rate.1,
                I::n_channels(),
                &mut err,
                &self.io_spec,
                &self.quality_spec,
                &self.runtime_spec,
            )
        };
        try!(unsafe{from_soxr_error(err)});
        Ok(Soxr{handle: res, _phantom: PhantomData})
    }
}

impl<I,O> Drop for Soxr<I,O> {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe { sys::soxr_delete(self.handle) };
            self.handle = ptr::null_mut();
        }
    }
}

impl<I: SoxrFormat, O: SoxrFormat>  Soxr<I,O> {
    pub fn process(&mut self, inbuf: Option<&[I]>, outbuf: &mut[I]) -> Result<usize, Error> {
        let mut odone = 0;
        try!(unsafe{from_soxr_error(
            sys::soxr_process(self.handle,
                              inbuf.map_or(ptr::null(), |x| x.as_ptr() as *const _),
                              inbuf.map_or(0, |x| x.len()),
                              ptr::null_mut(),

                              outbuf.as_mut_ptr() as *mut _,
                              outbuf.len(),
                              &mut odone)
        )});
        Ok(odone)
    }

    pub fn change_rate(&mut self, irate: f64, orate: f64, slew_len: usize) -> Result<(), Error> {
        unsafe{from_soxr_error(
            sys::soxr_set_io_ratio(self.handle, irate/orate, slew_len)
        )}
    }
}


#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}
