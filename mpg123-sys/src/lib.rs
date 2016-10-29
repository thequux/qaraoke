#[macro_use] extern crate bitflags;
#[macro_use] extern crate enum_primitive;

extern crate libc;

pub use libc::{c_void, c_char, c_int, c_long, c_ulong, size_t, c_double, off_t};
use std::convert::From;

#[link(name = "mpg123")]
extern {
    pub fn mpg123_init() -> c_int;
    pub fn mpg123_exit();
    pub fn mpg123_new(decoder: *const c_char, error: *mut c_int) -> *mut Mpg123Handle;
    pub fn mpg123_delete(handle: *mut Mpg123Handle);
    pub fn mpg123_param(handle: *mut Mpg123Handle, type_: Mpg123Param, value: c_long, fvalue: c_double) -> c_int;
    pub fn mpg123_getparam(handle: *mut Mpg123Handle, type_: Mpg123Param, value: *mut c_long, f_value: *mut c_double) -> c_int;
    pub fn mpg123_feature(feature: Mpg123Feature) -> c_int;

    // Error handling
    pub fn mpg123_plain_strerror(errcode: c_int) -> *const c_char;
    pub fn mpg123_strerror(handle: *mut Mpg123Handle) -> *const c_char;
    pub fn mpg123_errcode(handle: *mut Mpg123Handle) -> Mpg123Error;

    // Decoder selection
    pub fn mpg123_decoders() -> *const *const c_char;
    pub fn mpg123_supported_decoders() -> *const *const c_char;
    pub fn mpg123_decoder(handle: *mut Mpg123Handle) -> c_int;
    pub fn mpg123_current_decoder(handle: *mut Mpg123Handle) -> *const c_char;

    // Output format
    pub fn mpg123_rates(list: *mut *const c_long, count: *mut size_t);
    pub fn mpg123_encodings(list: *mut *const c_int, count: *mut size_t);
    pub fn mpg123_encsize(encoding: c_int) -> c_int;

    pub fn mpg123_format_none(handle: *mut Mpg123Handle) -> c_int;
    pub fn mpg123_format_all(handle: *mut Mpg123Handle) -> c_int;
    pub fn mpg123_format(handle: *mut Mpg123Handle, rate: c_int, channels: c_int, encodings: c_int) -> c_int;
    pub fn mpg123_format_support(handle: *mut Mpg123Handle, rate: c_int, encodings: c_int) -> c_int;
    pub fn mpg123_getformat(handle: *mut Mpg123Handle, rate: *mut c_long, channels: *mut c_int, encodings: *mut c_int) -> c_int;

    // File input and decoding
    pub fn mpg123_open(handle: *mut Mpg123Handle, path: *const c_char) -> c_int;
    pub fn mpg123_open_fd(handle: *mut Mpg123Handle, fd: c_int) -> c_int;
    pub fn mpg123_open_handle(handle: *mut Mpg123Handle, iohandle: *mut c_void) -> c_int;
    pub fn mpg123_open_feed(handle: *mut Mpg123Handle) -> c_int;
    pub fn mpg123_close(handle: *mut Mpg123Handle) -> c_int;

    pub fn mpg123_read(handle: *mut Mpg123Handle, outmem: *mut u8, memsize: size_t, done: *mut size_t) -> c_int;
    pub fn mpg123_feed(handle: *mut Mpg123Handle, mem: *const u8, size: size_t) -> c_int;
    pub fn mpg123_decode(handle: *mut Mpg123Handle, inmem: *const u8, insize: size_t, outmem: *mut u8, outsize: *mut size_t) -> c_int;
    pub fn mpg123_decode_frame(handle: *mut Mpg123Handle, num: *mut off_t, audio: *mut *const u8, bytes: *mut size_t) -> c_int;
    pub fn mpg123_framebyframe_decode(handle: *mut Mpg123Handle, num: *mut off_t, audio: *mut *const u8, bytes: *mut size_t) -> c_int;
    pub fn mpg123_framebyframe_next(handle: *mut Mpg123Handle) -> c_int;

    pub fn mpg123_framedata(handle: *mut Mpg123Handle, header: *mut c_ulong, bodydata: *mut *mut u8, bodybytes: *mut size_t) -> c_int;
    pub fn mpg123_framepos(handle: *mut Mpg123Handle) -> off_t;

    // Position and seeking
    pub fn mpg123_tell(handle: *mut Mpg123Handle) -> off_t;
    pub fn mpg123_tellframe(handle: *mut Mpg123Handle) -> off_t;
    pub fn mpg123_tell_stream(handle: *mut Mpg123Handle) -> off_t;

    pub fn mpg123_seek(handle: *mut Mpg123Handle, sampleoff: off_t, whence: c_int) -> off_t;
    pub fn mpg123_feedseek(handle: *mut Mpg123Handle, sampleoff: off_t, whence: c_int, input_offset: *mut off_t) -> off_t;
    pub fn mpg123_seek_frame(handle: *mut Mpg123Handle, frameoff: off_t, whence: c_int) -> off_t;
    pub fn mpg123_timeframe(handle: *mut Mpg123Handle, sec: c_double) -> off_t;

    pub fn mpg123_index(handle: *mut Mpg123Handle, offsets: *mut *const off_t, step: *mut off_t, fill: *mut size_t) -> c_int;
    pub fn mpg123_set_index(handle: *mut Mpg123Handle, offsets: *mut off_t, step: off_t, fill: size_t) -> c_int;

    // We leave off mpg123_position because it's not stable
    // Also everything after mpg123_eq

}


pub enum Mpg123Handle {}

enum_from_primitive!{
#[repr(C)]
pub enum Mpg123Param {
    Verbose,
    Flags,
    AddFlags,
    ForceRate,
    DownSample,
    Rva,
    Downspeed,
    Upspeed,
    StartFrame,
    DecodeFrames,
    IcyInternal,
    Outscale,
    Timeout,
    RemoveFlags,
    ResyncLimit,
    IndexSize,
    Preframes,
    Feedpool,
    Feedbuffer,
}
}
// Enum conversion:
// sed -Ee 's@^\s*,?MPG123_([^, ]*),?(\s*=\s*[-x0-9a-fA-F]+)?\s*/\*\*<[ 01]*(.*?)\s*\*/@/// \3\n\1\2,@' |sed -e 's/^/    /' -e 's/\s*$//'
// Bitflags conversion:
// sed -Ee 's@^\s*,?MPG123_([^ ]*)\s*=\s*(0x[0-9a-fA-F]+)\s*/\*\*<[ 01]*(.*?)\s*\*/@/// \3\nconst \1 = \2;@' |sed -e 's/^/        /'

bitflags!{
    // Contents generated using
    // sed -Ee 's@^\s*,?MPG123_([^ ]*)\s*=\s*(0x[0-9a-fA-F]+)\s*/\*\*<[ 01]*(.*?)\s*\*/@/// \3\nconst \1 = \2;@' |sed -e 's/^/        /'
    pub flags Mpg123ParamFlags: c_ulong {
        /// Force some mono mode: This is a test bitmask for seeing if
        /// any mono forcing is active.
        const FLAG_FORCE_MONO = 0x7,
        /// Force playback of left channel only.
        const FLAG_MONO_LEFT = 0x1,
        /// Force playback of right channel only.
        const FLAG_MONO_RIGHT = 0x2,
        /// Force playback of mixed mono.
        const FLAG_MONO_MIX = 0x4,
        /// Force stereo output.
        const FLAG_FORCE_STEREO = 0x8,
        /// Force 8bit formats.
        const FLAG_FORCE_8BIT = 0x10,
        /// Suppress any printouts (overrules verbose).
        const FLAG_QUIET = 0x20,
        /// Enable gapless decoding (default on if libmpg123 has
        /// support).
        const FLAG_GAPLESS = 0x40,
        /// Disable resync stream after error.
        const FLAG_NO_RESYNC = 0x80,
        /// Enable small buffer on non-seekable streams to allow some
        /// peek-ahead (for better MPEG sync).
        const FLAG_SEEKBUFFER = 0x100,
        /// Enable fuzzy seeks (guessing byte offsets or using
        /// approximate seek points from Xing TOC)
        const FLAG_FUZZY = 0x200,
        /// Force floating point output (32 or 64 bits depends on
        /// mpg123 internal precision).
        const FLAG_FORCE_FLOAT = 0x400,
        /// Do not translate ID3 text data to UTF-8. ID3 strings will
        /// contain the raw text data, with the first byte containing
        /// the ID3 encoding code.
        const FLAG_PLAIN_ID3TEXT = 0x800,
        /// Ignore any stream length information contained in the
        /// stream, which can be contained in a 'TLEN' frame of an
        /// ID3v2 tag or a Xing tag
        const FLAG_IGNORE_STREAMLENGTH = 0x1000,
        /// Do not parse ID3v2 tags, just skip them.
        const FLAG_SKIP_ID3V2 = 0x2000,
        /// Do not parse the LAME/Xing info frame, treat it as normal
        /// MPEG data.
        const FLAG_IGNORE_INFOFRAME = 0x4000,
        /// Allow automatic internal resampling of any kind (default
        /// on if supported). Especially when going lowlevel with
        /// replacing output buffer, you might want to unset this
        /// flag. Setting MPG123_DOWNSAMPLE or MPG123_FORCE_RATE will
        /// override this.
        const FLAG_AUTO_RESAMPLE = 0x8000,
        /// 7th bit: Enable storage of pictures from tags (ID3v2 APIC).
        const FLAG_PICTURE = 0x10000,
    }
}

enum_from_primitive!{
#[repr(u64)]
pub enum ParamRVA {
    RvaOff,
    RvaMix,
    RvaAlbum,
}
}

// generated with
// sed -Ee 's@^\s*,?MPG123_([^ ]*)(\s*=\s*[x0-9a-fA-F]+)?\s*/\*\*<[ 01]*(.*?)\s*\*/@/// \3\n\1\2,@' |sed -e 's/^/        /'
#[repr(u64)]
pub enum Mpg123Feature {
        /// mpg123 expects path names to be given in UTF-8 encoding instead of plain native.
        AbiUtf8Open,
        /// 8bit output
        Output8bit,
        /// 6bit output
        Output16bit,
        /// 32bit output
        Output32bit,
        /// support for building a frame index for accurate seeking
        Index,
        /// id3v2 parsing
        ParseID3v2,
        /// mpeg layer-1 decoder enabled
        DecodeLayer1,
        /// mpeg layer-2 decoder enabled
        DecodeLayer2,
        /// mpeg layer-3 decoder enabled
        DecodeLayer3,
        /// accurate decoder rounding
        DecodeAccurate,
        /// downsample (sample omit)
        DecodeDownsample,
        /// flexible rate decoding
        DecodeNtoM,
        /// ICY support
        ParseICY,
        /// Reader with timeout (network).
        TimeoutRead,
}

#[repr(i32)]
#[derive(Copy,Clone,Debug,PartialEq)]
pub enum Mpg123Error {
    /// Message: Track ended. Stop decoding. 
    Done = -12,
    /// Message: Output format will be different on next call. Note
    /// that some libmpg123 versions between 1.4.3 and 1.8.0 insist on
    /// you calling mpg123_getformat() after getting this message
    /// code. Newer verisons behave like advertised: You have the
    /// chance to call mpg123_getformat(), but you can also just
    /// continue decoding and get your data.
    NewFormat = -11,
    /// Message: For feed reader: "Feed me more!" (call mpg123_feed()
    /// or mpg123_decode() with some new input data).
    NeedMore = -10,
    /// Generic Error 
    Err = -1,
    /// Success 
    Ok = 0,
    /// Unable to set up output format! 
    BadOutFormat = 1,
    /// Invalid channel number specified. 
    BadChannel = 2,
    /// Invalid sample rate specified.  
    BadRate = 3,
    /// Unable to allocate memory for 16 to 8 converter table! 
    Err16to8Table = 4,
    /// Bad parameter id! 
    BadParam = 5,
    /// Bad buffer given -- invalid pointer or too small size. 
    BadBuffer = 6,
    /// Out of memory -- some malloc() failed. 
    OutOfMem = 7,
    /// You didn't initialize the library! 
    NotInitialized = 8,
    /// Invalid decoder choice. 
    BadDecoder = 9,
    /// Invalid mpg123 handle. 
    BadHandle = 10,
    /// Unable to initialize frame buffers (out of memory?). 
    NoBuffers = 11,
    /// Invalid RVA mode. 
    BadRva = 12,
    /// This build doesn't support gapless decoding. 
    NoGapless = 13,
    /// Not enough buffer space. 
    NoSpace = 14,
    /// Incompatible numeric data types. 
    BadTypes = 15,
    /// Bad equalizer band. 
    BadBand = 16,
    /// Null pointer given where valid storage address needed. 
    ErrNull = 17,
    /// Error reading the stream. 
    ErrReader = 18,
    /// Cannot seek from end (end is not known). 
    NoSeekFromEnd = 19,
    /// Invalid 'whence' for seek function.
    BadWhence = 20,
    /// Build does not support stream timeouts. 
    NoTimeout = 21,
    /// File access error. 
    BadFile = 22,
    /// Seek not supported by stream. 
    NoSeek = 23,
    /// No stream opened. 
    NoReader = 24,
    /// Bad parameter handle. 
    BadPars = 25,
    /// Bad parameters to mpg123_index() and mpg123_set_index() 
    BadIndexPar = 26,
    /// Lost track in bytestream and did not try to resync. 
    OutOfSync = 27,
    /// Resync failed to find valid MPEG data. 
    ResyncFail = 28,
    /// No 8bit encoding possible. 
    No8bit = 29,
    /// Stack aligmnent error 
    BadAlign = 30,
    /// Null input buffer with non-zero size... 
    NullBuffer = 31,
    /// Relative seek not possible (screwed up file offset) 
    NoRelseek = 32,
    /// You gave a null pointer somewhere where you shouldn't have.
    NullPointer = 33,
    /// Bad key value given. 
    BadKey = 34,
    /// No frame index in this build. 
    NoIndex = 35,
    /// Something with frame index went wrong. 
    IndexFail = 36,
    /// Something prevents a proper decoder setup 
    BadDecoderSetup = 37,
    /// This feature has not been built into libmpg123. 
    MissingFeature = 38,
    /// A bad value has been given, somewhere. 
    BadValue = 39,
    /// Low-level seek failed. 
    LseekFailed = 40,
    /// Custom I/O not prepared. 
    BadCustomIo = 41,
    /// Offset value overflow during translation of large file API
    /// calls -- your client program cannot handle that large file.
    LfsOverflow = 42,
    /// Some integer overflow. 
    IntOverflow = 43,
}

impl From<c_int> for Mpg123Error {
    fn from(v: c_int) -> Self {
        use Mpg123Error::*;
        match v {
            -12 => Done,
            -11 => NewFormat,
            -10 => NeedMore,
            -1 => Err,
            0 => Ok,
            1 => BadOutFormat,
            2 => BadChannel,
            3 => BadRate,
            4 => Err16to8Table,
            5 => BadParam,
            6 => BadBuffer,
            7 => OutOfMem,
            8 => NotInitialized,
            9 => BadDecoder,
            10 => BadHandle,
            11 => NoBuffers,
            12 => BadRva,
            13 => NoGapless,
            14 => NoSpace,
            15 => BadTypes,
            16 => BadBand,
            17 => ErrNull,
            18 => ErrReader,
            19 => NoSeekFromEnd,
            20 => BadWhence,
            21 => NoTimeout,
            22 => BadFile,
            23 => NoSeek,
            24 => NoReader,
            25 => BadPars,
            26 => BadIndexPar,
            27 => OutOfSync,
            28 => ResyncFail,
            29 => No8bit,
            30 => BadAlign,
            31 => NullBuffer,
            32 => NoRelseek,
            33 => NullPointer,
            34 => BadKey,
            35 => NoIndex,
            36 => IndexFail,
            37 => BadDecoderSetup,
            38 => MissingFeature,
            39 => BadValue,
            40 => LseekFailed,
            41 => BadCustomIo,
            42 => LfsOverflow,
            43 => IntOverflow,
            _ => Err,
        }
    }
}

// This encoding is disasterous, but we have what we have.
bitflags!{
    pub flags Enc : i32 {
        const ENC_8 = 0x00f,
        const ENC_16 = 0x040,
        const ENC_24 = 0x4000,
        const ENC_32 = 0x100,
        const ENC_SIGNED = 0x080,
        const ENC_FLOAT = 0xe00,
        // Specific formats
        const ENC_UNSIGNED_8 = 0x01,
        const ENC_SIGNED_8 = ENC_SIGNED.bits | 0x02,
        const ENC_ULAW_8 = 0x04,
        const ENC_ALAW_8 = 0x08,
        const ENC_SIGNED_16 = 0x10 | ENC_16.bits | ENC_SIGNED.bits,
        const ENC_UNSIGNED_16 = 0x20 | ENC_16.bits,
        const ENC_SIGNED_32 = 0x1000 | ENC_32.bits | ENC_SIGNED.bits,
        const ENC_UNSIGNED_32 = 0x2000 | ENC_32.bits,
        const ENC_SIGNED_24 = 0x1000 | ENC_24.bits | ENC_SIGNED.bits,
        const ENC_UNSIGNED_24 = 0x2000 | ENC_24.bits,
        const ENC_FLOAT_32 = 0x200,
        const ENC_FLOAT_64 = 0x400,

        const ENC_ANY = (ENC_UNSIGNED_8.bits | ENC_SIGNED_8.bits
                         | ENC_ULAW_8.bits | ENC_ALAW_8.bits
                         | ENC_SIGNED_16.bits | ENC_UNSIGNED_16.bits
                         | ENC_SIGNED_32.bits | ENC_UNSIGNED_32.bits
                         | ENC_SIGNED_24.bits | ENC_UNSIGNED_24.bits
                         | ENC_FLOAT_32.bits | ENC_FLOAT_64.bits),
    }
}

impl Enc {
    /// Return the number of bytes per mono sample
    pub fn size(&self) -> usize {
        unsafe {
            mpg123_encsize(self.bits()) as usize
        }
    }
}

bitflags!{
    pub flags ChannelCount : i32 {
        const CHAN_MONO = 1,
        const CHAN_STEREO = 2,
    }
}

