use std::io::prelude::*;
use std::io;
// use std::cell::RefCell;
//use std::collections::VecDeque;

use ogg;
use util;

pub struct Mp3Stream<R> {
    reader: R,
    buffer: util::ShiftBuffer,
}

impl <R: Read> Mp3Stream<R> {
    pub fn new(reader: R) -> Self {
        // This will fail if the buffer is not large enough to contain
        // the largest complete frame
        Mp3Stream{
            reader: reader,
            // The largest possible mp3 frame is 2881 bytes.
            buffer: util::ShiftBuffer::new(2881),
        }
    }

    pub fn next_frame(&mut self) -> io::Result<Option<&[u8]>> {
        loop {
            try!(self.buffer.fill_max(&mut self.reader));
            // find the beginning of a frame
            if self.buffer.is_empty() {
                // Must have been an EOF
                return Ok(None);
            }
            let mut frame_found = false;
            for i in 0..self.buffer.len()-1 {
                // This is just layer III. To match layers I and II as well, it should be
                // && self.buffer[i+1] & 0xE0 == 0xE0
                // TODO: support encoding audio layers I and II
                if self.buffer[i] == 0xFF && self.buffer[i+1] & 0xE6 == 0xE2 {
                    frame_found = true;
                    if i != 0 {
                        self.buffer.consume(i);
                    }
                    if self.buffer.len() < 4 {
                        try!(self.buffer.fill_max(&mut self.reader));
                    }
                    break;
                }
            }
            if !frame_found {
                let len = self.buffer.len();
                self.buffer.consume(len);
                continue;
            }
            // Validate the frame.
            match mpg_get_frame_size(&self.buffer[0..4]) {
                Some(len) => return Ok(Some(self.buffer.consume(len))),
                None => {
                    // false match
                    self.buffer.consume(1);
                    continue;
                },
            }
        }
    }
}


// Frame sizing calculation, stolen from https://hydrogenaud.io/index.php/topic,85125.0.html
// MPEG versions - use [version]
#[allow(unused)]
const MPEG_VERSIONS : [u8;4] = [ 25, 0, 2, 1 ];

// Layers - use [layer]
#[allow(unused)]
const MPEG_LAYERS : [u8;4] = [ 0, 3, 2, 1 ];

// Bitrates - use [version][layer][bitrate]
const MPEG_BITRATES : [[[u16;16];4];4] = [
  [ // Version 2.5
    [ 0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0, 0 ], // Reserved
    [ 0,   8,  16,  24,  32,  40,  48,  56,  64,  80,  96, 112, 128, 144, 160, 0 ], // Layer 3
    [ 0,   8,  16,  24,  32,  40,  48,  56,  64,  80,  96, 112, 128, 144, 160, 0 ], // Layer 2
    [ 0,  32,  48,  56,  64,  80,  96, 112, 128, 144, 160, 176, 192, 224, 256, 0 ]  // Layer 1
  ],
  [ // Reserved
    [ 0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0, 0 ], // Invalid
    [ 0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0, 0 ], // Invalid
    [ 0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0, 0 ], // Invalid
    [ 0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0, 0 ]  // Invalid
  ],
  [ // Version 2
    [ 0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0, 0 ], // Reserved
    [ 0,   8,  16,  24,  32,  40,  48,  56,  64,  80,  96, 112, 128, 144, 160, 0 ], // Layer 3
    [ 0,   8,  16,  24,  32,  40,  48,  56,  64,  80,  96, 112, 128, 144, 160, 0 ], // Layer 2
    [ 0,  32,  48,  56,  64,  80,  96, 112, 128, 144, 160, 176, 192, 224, 256, 0 ]  // Layer 1
  ],
  [ // Version 1
    [ 0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0, 0 ], // Reserved
    [ 0,  32,  40,  48,  56,  64,  80,  96, 112, 128, 160, 192, 224, 256, 320, 0 ], // Layer 3
    [ 0,  32,  48,  56,  64,  80,  96, 112, 128, 160, 192, 224, 256, 320, 384, 0 ], // Layer 2
    [ 0,  32,  64,  96, 128, 160, 192, 224, 256, 288, 320, 352, 384, 416, 448, 0 ], // Layer 1
  ]
];

// Sample rates - use [version][srate]
const MPEG_SRATES : [[u16;4];4] = [
    [ 11025, 12000,  8000, 0 ], // MPEG 2.5
    [     0,     0,     0, 0 ], // Reserved
    [ 22050, 24000, 16000, 0 ], // MPEG 2
    [ 44100, 48000, 32000, 0 ]  // MPEG 1
];

// Samples per frame - use [version][layer]
const MPEG_FRAME_SAMPLES : [[u16;4];4] = [
//    Rsvd     3     2     1  < Layer  v Version
    [    0,  576, 1152,  384 ], //       2.5
    [    0,    0,    0,    0 ], //       Reserved
    [    0,  576, 1152,  384 ], //       2
    [    0, 1152, 1152,  384 ]  //       1
];

// Slot size (MPEG unit of measurement) - use [layer]
const MPEG_SLOT_SIZE : [u16;4] = [ 0, 1, 1, 4 ]; // Rsvd, 3, 2, 1


fn mpg_get_frame_size (hdr: &[u8]) -> Option<usize> {
    
    // Quick validity check
    if     ( (hdr[0] & 0xFF) != 0xFF)
        || ( (hdr[1] & 0xE0) != 0xE0)   // 3 sync bits
        || ( (hdr[1] & 0x18) == 0x08)   // Version rsvd
        || ( (hdr[1] & 0x06) == 0x00)   // Layer rsvd
        || ( (hdr[2] & 0xF0) == 0xF0)   // Bitrate rsvd
    {
        return None;
    }
    
    // Data to be extracted from the header
    let ver = ((hdr[1] & 0x18) >> 3) as usize;   // Version index
    let lyr = ((hdr[1] & 0x06) >> 1) as usize;   // Layer index
    let pad = ((hdr[2] & 0x02) >> 1) as usize;   // Padding? 0/1
    let brx = ((hdr[2] & 0xf0) >> 4) as usize;   // Bitrate index
    let srx = ((hdr[2] & 0x0c) >> 2) as usize;   // SampRate index
    
    // Lookup real values of these fields
    let bitrate   = MPEG_BITRATES[ver][lyr][brx] as usize * 1000;
    let samprate  = MPEG_SRATES[ver][srx] as usize;
    let samples   = MPEG_FRAME_SAMPLES[ver][lyr] as usize;
    let slot_size = MPEG_SLOT_SIZE[lyr] as usize;
    
    if samprate == 0 {
        return None;
    }
    let base_framesize = (samples * bitrate / samprate) >> 3;

    // Frame sizes are truncated integers
    if pad == 1 {
	Some(base_framesize + slot_size)
    } else {
	Some(base_framesize)
    }
}

pub fn max_fsize() -> usize {
    use std::cmp::max;
    let mut frame = [0xFF;3];
    let mut maxsize = 0;
    for x in 0..255 {
        frame[1] = x;
        for y in 0..255 {
            frame[2] = y;
            maxsize = max(maxsize, mpg_get_frame_size(&frame).unwrap_or(0));
        }
    }
    maxsize
}

// OggMP3 encoder
pub struct OggMP3Coder<R> {
    /// A reader that produces MP3 frames
    stream: Mp3Stream<R>,
    // Only Some until the first data frame has been produced
    first_frame: Option<ogg::Packet>,
    pseudoheader: [u8;4],
    samples_per_frame: u32,
    sample_frequency: u32,
    last_sample_no: u64,
}

impl <R: Read> OggMP3Coder<R> {
    pub fn new(reader: R) -> io::Result<Self> {
        let mut stream = Mp3Stream::new(reader);
        let first_frame = try!(stream.next_frame()).map(|frame| ogg::Packet{
            content: frame.to_owned(),
            timestamp: 0,
        });
        match first_frame {
            None => Err(io::Error::new(io::ErrorKind::UnexpectedEof, "MP3 file contained no valid frames")),
            Some(frame) => {
                // The pseudoheader is the frame header with the first byte set to 0.
                let pseudoheader = [0, frame.content[1], frame.content[2], frame.content[3]];
                let mp_ver = pseudoheader[1] as usize & 0x18 >> 3;
                let mp_lyr = pseudoheader[1] as usize & 0x06 >> 1;
                let mp_srx = pseudoheader[2] as usize & 0x0c >> 2;
                let sample_frequency = MPEG_SRATES[mp_ver][mp_srx] as u32;
                let samples_per_frame = MPEG_FRAME_SAMPLES[mp_ver][mp_lyr] as u32;

                Ok(OggMP3Coder{
                    stream: stream,
                    first_frame: Some(frame),
                    pseudoheader: pseudoheader,
                    sample_frequency: sample_frequency,
                    samples_per_frame: samples_per_frame,
                    last_sample_no: 0,
                })
            }
        }
    }
}

impl <R: Read> ogg::BitstreamCoder for OggMP3Coder<R> {
    fn headers(&self) -> Vec<Vec<u8>> {
        use byteorder::{LittleEndian,WriteBytesExt};
        let mut header = Vec::with_capacity(24);
        header.extend_from_slice(b"OggMP3\0\0");
        header.push(0); // major version
        header.push(0); // minor version
        header.push(if self.pseudoheader[3] & 0xC0 == 0xC0 { 0 } else { 2 }); // tag; we only care about the stereo bit
        header.push(0);
        header.extend_from_slice(&self.pseudoheader);

        header.write_u32::<LittleEndian>(self.sample_frequency).unwrap();
        header.write_u32::<LittleEndian>(self.samples_per_frame).unwrap();

        vec![header]
    }

    fn next_frame(&mut self) -> io::Result<Option<ogg::Packet>> {
        if self.last_sample_no == 0 {
            self.last_sample_no = self.samples_per_frame as u64;
            Ok(self.first_frame.take().map(|mut frame| {frame.timestamp = self.last_sample_no; frame}))
        } else {
            self.last_sample_no += self.samples_per_frame as u64;
            let next_frame = self.last_sample_no;
            self.stream.next_frame().map( |r| r.map(|frame| {
                ogg::Packet{
                    content: frame.to_owned(),
                    timestamp: next_frame,
                }
            }))
        }
    }

    fn map_granule(&self, timestamp: u64) -> u64 {
        timestamp * 1000_000 / self.sample_frequency as u64
    }
}
