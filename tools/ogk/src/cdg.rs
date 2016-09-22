use std::io::prelude::*;
use std::io;

use lz4;
use ogg;

pub struct OggCdgCoder<R> {
    reader: R,
    packetsize: u8,
    // TODO: add keyframe support
    cur_frame: u64,
    last_keyframe: u64,
}

impl <R: Read> OggCdgCoder<R> {
    pub fn new(reader: R) -> Self {
        OggCdgCoder{
            reader: reader,
            packetsize: 75,
            cur_frame: 0,
            last_keyframe: 0,
        }
    }
}

impl <R: Read> ogg::BitstreamCoder for OggCdgCoder<R> {
    //type Frame = Frame;
    //type Error = io::Error;
    
    fn headers(&self) -> Vec<Vec<u8>> {
        let mut header = Vec::with_capacity(14);

        header.extend_from_slice(b"OggCDG\0\0");
        header.push(0);
        header.push(0);
        header.push(1); // LZ4
        header.push(self.packetsize);
        vec![header]
    }

    fn next_frame(&mut self) -> io::Result<Option<ogg::Packet>> {
        let mut input = Vec::with_capacity(self.packetsize as usize * 96);
        let mut output = Vec::new();
        let size = try!(self.reader.by_ref().take(self.packetsize as u64 * 96).read_to_end(&mut input));
        //let size = try!(self.reader.read(&mut input));
        if size % 96 != 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Incomplete sector read"));
        }

        if size == 0 {
            return Ok(None);
        }
        
        output.push(0);
        output.push((size / 96) as u8);

        let mut encoder = try!(lz4::EncoderBuilder::new()
                               .level(9)
                               .checksum(lz4::ContentChecksum::NoChecksum)
                               .build(output));
        
        try!(encoder.write_all(&input));
        let (output, result) = encoder.finish();
        try!(result);
        
        self.cur_frame += size as u64 / 96;

        Ok(Some(ogg::Packet{
            content: output,
            timestamp: self.cur_frame << 32 | self.last_keyframe,
        }))
    }

    fn map_granule(&self, granule: u64) -> u64 {
        (granule >> 32) * 1000_000 / 75
    }
}
